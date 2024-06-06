use std::{fs::{read_to_string, write, File}, path::{Path, PathBuf}, process::Command, time::Duration};

use candid::{Decode, Encode};
use ic_agent::{export::Principal, Agent};
use like_shell::{run_successful_command, temp_dir_from_template, Capture, TemporaryChild};
// use dotenv::dotenv;
use tempdir::TempDir;
use tokio::time::sleep;
use toml_edit::{value, DocumentMut};
use anyhow::Context;
use serde_json::Value;

// TODO: Check this file for logical inconsistencies and like this.

struct Test {
    dir: TempDir,
    // cargo_manifest_dir: PathBuf,
    workspace_dir: PathBuf,
}

impl Test {
    pub async fn new(tmpl_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let cargo_manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_dir = cargo_manifest_dir.join("..").join("..");
        let dir = temp_dir_from_template(tmpl_dir)?;

        let res = Self {
            dir,
            // cargo_manifest_dir: cargo_manifest_dir.to_path_buf(),
            workspace_dir: workspace_dir,
        };

        Ok(res)
    }
}

// TODO: Should have more abstract DFXDir.
struct OurDFX<'a> {
    pub base: &'a Test,
    test_canister_id: Principal,
    agent: Agent,
}

impl<'a> OurDFX<'a> {
    pub async fn new(base: &'a Test, additional_args: &[&str]) -> Result<Self, Box<dyn std::error::Error>> {
        // TODO: Specifying a specific port is a hack.
        run_successful_command(&mut Command::new(
            "/root/.local/share/dfx/bin/dfx" // TODO: Split base.dir.path().
        ).args([&["start", "--host", "127.0.0.1:8007", "--background"] as &[&str], additional_args].concat()).current_dir(base.dir.path()))
            .context("Starting DFX")?;

        let port_str = read_to_string(
            base.dir.path().join(".dfx").join("network").join("local").join("webserver-port"),
        ).context("Reading port.")?;
        let port: u16 = port_str.parse().context("Parsing port number.")?;

        println!("Connecting to DFX (port {port})");
        run_successful_command(Command::new(
            "/root/.local/share/dfx/bin/dfx" // TODO: Split base.dir.path().
        ).args(["deploy"]))?;
        // dotenv().ok();

        let canister_ids: Value = {
            let dir = base.dir.path().join(".dfx").join("local").join("canister_ids.json");
            let file = File::open(dir).with_context(|| format!("Opening canister_ids.json"))?;
            serde_json::from_reader(file).expect("Error parsing JSON")
        };
        let test_canister_id = canister_ids.as_object().unwrap()["test"].as_object().unwrap()["local"].as_str().unwrap();
        let call_canister_id = canister_ids.as_object().unwrap()["call"].as_object().unwrap()["local"].as_str().unwrap();

        let agent = Agent::builder().with_url(format!("http://127.0.0.1:{port}")).build().context("Creating Agent")?;
        agent.fetch_root_key().await.context("Fetching root keys.")?; // DON'T USE this on mainnet

        let toml_path = base.dir.path().join("config.toml");
        let toml = read_to_string(&toml_path).context("Reading config.")?;
        let mut doc = toml.parse::<DocumentMut>().context("Invalid TOML")?;
        doc["callback"]["canister"] = value(call_canister_id.to_string());
        write(&toml_path, doc.to_string()).context("Writing modified config.")?;

        Ok(Self {
            base: &base,
            test_canister_id: Principal::from_text(test_canister_id)
                .context("Parsing principal")?,
            agent,
        })
    }
}

impl<'a> Drop for OurDFX<'a> {
    fn drop(&mut self) {
        run_successful_command(&mut Command::new(
            "/root/.local/share/dfx/bin/dfx" // TODO: Split path.
        ).args(["stop"]).current_dir(self.base.dir.path()))
            .context("Stopping DFX").expect("can't stop DFX");
    }
}

async fn test_calls<'a>(test: &'a OurDFX<'a>, path: &str, arg: &str, body: &str) -> Result<(), Box<dyn std::error::Error>> {
    let res =
        test.agent.update(&test.test_canister_id, "test").with_arg(Encode!(&path, &arg, &body)?)
            .call_and_wait().await?;//.context("Call to IC.")?;
    // println!("[[{:?}]]", res.map(|s| String::from_utf8_lossy(&s).to_string())); // TODO: Remove.
    assert_eq!(
        Decode!(&res, String).context("Decoding test call response.")?,
        format!("path={}&arg={}&body={}", path, arg, body),
    );
    // TODO: Check two/three parallel requests.
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cargo_manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tmpl_dir = cargo_manifest_dir.join("tmpl");

    let test = Test::new(&tmpl_dir).await?;
    let _test_http = TemporaryChild::spawn(&mut Command::new(
        test.workspace_dir.join("target").join("debug").join("test-server")
    ), Capture { stdout: None, stderr: None }).context("Running test HTTPS server")?;
    let _proxy = TemporaryChild::spawn(&mut Command::new(
        test.workspace_dir.join("target").join("debug").join("joining-proxy")
    ).current_dir(test.dir.path()), Capture { stdout: None, stderr: None }).context("Running Joining Proxy")?;
    sleep(Duration::from_millis(2000)).await; // Wait till daemons start.

    // Test both small and bigartificial delay:
    {
        let dfx = OurDFX::new(&test, &["--artificial-delay", "0"]).await?;
        test_calls(&dfx, "/qq", "zz", "yu").await?;
    }
    {
        let dfx = OurDFX::new(&test, &["--artificial-delay", "5000"]).await?;
        test_calls(&dfx, "/qq", "zz", "yu").await?;
    }

    // TODO: Test that varying every one of three step parameters causes Miss.
    Ok(())
}