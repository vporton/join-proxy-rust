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
struct OurDFX {
    pub base: Test,
    test_canister_id: Principal,
    agent: Agent,
}

impl OurDFX {
    pub async fn new(tmpl_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let base = Test::new(tmpl_dir).await?;

        // TODO: Specifying a specific port is a hack.
        run_successful_command(&mut Command::new(
            "/root/.local/share/dfx/bin/dfx" // TODO: Split path.
        ).args(["start", "--host", "127.0.0.1:8007", "--background"]).current_dir(base.dir.path()))
            .context("Starting DFX")?;

        let port_str = read_to_string(
            base.dir.path().join(".dfx").join("network").join("local").join("webserver-port"),
        ).context("Reading port.")?;
        let port: u16 = port_str.parse().context("Parsing port number.")?;

        println!("Connecting to DFX (port {port})");
        run_successful_command(Command::new(
            "/root/.local/share/dfx/bin/dfx" // TODO: Split path.
        ).args(["deploy"]))?;
        // dotenv().ok();

        let canister_ids: Value = {
            let path = base.dir.path().join(".dfx").join("local").join("canister_ids.json");
            let file = File::open(path).with_context(|| format!("Opening canister_ids.json"))?;
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
            base,
            test_canister_id: Principal::from_text(test_canister_id)
                .context("Parsing principal")?,
            // call_canister_id: Principal::from_text(call_canister_id)
            //     .context("Parsing principal")?,
            agent,
        })
    }
}

impl Drop for OurDFX {
    fn drop(&mut self) {
        run_successful_command(&mut Command::new(
            "/root/.local/share/dfx/bin/dfx" // TODO: Split path.
        ).args(["stop"]).current_dir(self.base.dir.path()))
            .context("Stopping DFX").expect("can't stop DFX");
    }
}

async fn test_calls(test: &OurDFX, path: &str, arg: &str) -> Result<(), Box<dyn std::error::Error>> {
    let body = "";
    let farg = Encode!(&path.to_string(), &arg.to_string())?; // FIXME
    let res =
        test.agent.update(&test.test_canister_id, "test").with_arg(farg)
            .call_and_wait().await.context("Call to IC.")?;
    assert_eq!(
        Decode!(&res, String).context("Decoding test call response.")?,
        format!("path={}&arg={}&body={}", path, arg, body),
    );
    // TODO: Check two parallel requests.
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cargo_manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tmpl_dir = cargo_manifest_dir.join("tmpl");

    let test = OurDFX::new(&tmpl_dir).await?;
    let _test_http = TemporaryChild::spawn(&mut Command::new(
        test.base.workspace_dir.join("target").join("debug").join("test-server")
    ), Capture { stdout: None, stderr: None }).context("Running test HTTPS server")?;
    let _proxy = TemporaryChild::spawn(&mut Command::new(
        test.base.workspace_dir.join("target").join("debug").join("joining-proxy")
    ).current_dir(test.base.dir.path()), Capture { stdout: None, stderr: None }).context("Running Joining Proxy")?;
    sleep(Duration::from_millis(1000)).await; // Wait till daemons start.
    test_calls(&test, "/qq", "zz").await?;
    // TODO
    Ok(())
}