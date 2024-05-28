use std::{fs::{read_to_string, write, File}, path::{Path, PathBuf}, process::Command, time::Duration};

use candid::{Decode, Encode};
use fs_extra::{dir, file};
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
    agent: Agent,
    call_canister_id: Principal,
    test_canister_id: Principal,
}

impl Test {
    pub async fn new(tmpl_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let cargo_manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_dir = cargo_manifest_dir.join("..").join("..");
        let dir = temp_dir_from_template(tmpl_dir)?;
        dir::copy(workspace_dir.join("motoko"), dir.path(), &dir::CopyOptions::new())
            .context("Copying files.")?; // FIXME: What should be `copy_inside` value?
        file::copy(
            workspace_dir.join("mops.toml"),
            dir.path().join("mops.toml"),
            &file::CopyOptions::new(),
        ).context("Copying a file.")?;
    
        // TODO: Specifying a specific port is a hack.
        let _dfx_daemon = TemporaryChild::spawn(&mut Command::new(
            "dfx"
        ).args(["start", "--host", "127.0.0.1:8007"]).current_dir(dir.path()), Capture { stdout: None, stderr: None })
            .context("Starting DFX")?;
        sleep(Duration::from_millis(1000)).await; // Wait till daemons start.
        run_successful_command(Command::new("mops").arg("install").current_dir(dir.path()))
            .context("Installing MOPS packages.")?;
        run_successful_command(Command::new("dfx").arg("deploy").current_dir(dir.path()))
            .context("Deploying.")?;
        // dotenv().ok();

        let port_str = read_to_string(
            dir.path().join(".dfx").join("network").join("local").join("webserver-port"),
        ).context("Reading port.")?;
        let port: u16 = port_str.parse().context("Parsing port number.")?;

        let canister_ids: Value = {
            let path = dir.path().join(".dfx").join("local").join("canister_ids.json");
            let file = File::open(path).with_context(|| format!("Opening canister_ids.json"))?;
            serde_json::from_reader(file).expect("Error parsing JSON")
        };
        let call_canister_id = canister_ids.as_object().unwrap()["call"].as_object().unwrap()["local"].as_str().unwrap();
        let test_canister_id = canister_ids.as_object().unwrap()["test"].as_object().unwrap()["local"].as_str().unwrap();

        println!("Connecting to port {port}");
        let res = Self {
            dir,
            agent: Agent::builder().with_url(format!("http://127.0.0.1:{port}")).build().context("Creating Agent")?,
            call_canister_id: Principal::from_text(call_canister_id)
                .context("Parsing principal")?,
            test_canister_id: Principal::from_text(test_canister_id)
                .context("Parsing principal")?,
            // cargo_manifest_dir: cargo_manifest_dir.to_path_buf(),
            workspace_dir: workspace_dir,
        };
        res.agent.fetch_root_key().await.context("Fetching root keys.")?; // DON'T USE this on mainnet

        let toml_path = res.dir.path().join("config.toml");
        let toml = read_to_string(&toml_path).context("Reading config.")?;
        let mut doc = toml.parse::<DocumentMut>().context("Invalid TOML")?;
        doc["callback"]["canister"] = value(res.call_canister_id.to_string());
        write(&toml_path, doc.to_string()).context("Writing modified config.")?;

        Ok(res)
    }
}

async fn test_calls(test: &Test) -> Result<(), Box<dyn std::error::Error>> {
    for add_host in [false, true] {
        let res =
            test.agent.update(&test.test_canister_id, "test").with_arg(Encode!(&add_host).unwrap())
                .call_and_wait().await.context("Back-call to IC.")?;
        assert_eq!(Decode!(&res, String).context("Decoding test call response.")?, "Test");
        // TODO: Check two parallel requests.
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cargo_manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tmpl_dir = cargo_manifest_dir.join("tmpls").join("basic");

    let test = Test::new(&tmpl_dir).await?;
    let _test_http = TemporaryChild::spawn(&mut Command::new(
        test.workspace_dir.join("target").join("debug").join("test-server")
    ), Capture { stdout: None, stderr: None })?;
    let _proxy = TemporaryChild::spawn(&mut Command::new(
        test.workspace_dir.join("target").join("debug").join("joining-proxy")
    ), Capture { stdout: None, stderr: None })?;
    sleep(Duration::from_millis(1000)).await; // Wait till daemons start.
    test_calls(&test).await?;
    // TODO
    Ok(())
}