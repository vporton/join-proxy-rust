use std::{fs::{read_to_string, write}, path::Path, process::Command, time::Duration};

use candid::{Decode, Encode};
use ic_agent::{export::Principal, Agent};
use like_shell::{run_successful_command, temp_dir_from_template, Capture, TemporaryChild};
use dotenv::{dotenv, var};
use tempdir::TempDir;
use tokio::time::sleep;
use toml_edit::{value, DocumentMut};

struct Test {
    dir: TempDir,
    agent: Agent,
    test_canister_id: Principal,
}

impl Test {
    pub async fn new(tmpl_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let res = Self {
            dir: temp_dir_from_template(tmpl_dir)?,
            agent: Agent::builder().with_url("http://localhost:39143").build()?,
            test_canister_id: Principal::from_text(var("CANISTER_ID_TEST")?)?,
        };

        res.agent.fetch_root_key().await?; // DON'T USE this on mainnet

        let toml_path = res.dir.path().join("config.toml");
        let toml = read_to_string(&toml_path)?;
        let mut doc = toml.parse::<DocumentMut>().expect("invalid doc");
        doc["callback"]["canister"] = value(var("CANISTER_ID_TEST")?);
        write(&toml_path, doc.to_string())?;

        Ok(res)
    }
}

async fn test_calls(test: &Test) -> Result<(), Box<dyn std::error::Error>> {
    for add_host in [false, true] {
        let res =
            test.agent.update(&test.test_canister_id, "test").with_arg(Encode!(&add_host)?)
                .call_and_wait().await?;
        assert_eq!(Decode!(&res, String)?, "Test");
        // TODO: Check two parallel requests.
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cargo_manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    dotenv().ok();

    // Before `temp_dir_from_template()`, because that changes the current dir:
    run_successful_command(Command::new("dfx").args(["deploy"]))?;

    let workspace_dir = cargo_manifest_dir.join("..").join("..");
    let tmpl_dir = cargo_manifest_dir.join("tmpls").join("basic");
    let test = Test::new(&tmpl_dir).await?;
    let _test_http = TemporaryChild::spawn(&mut Command::new(
        workspace_dir.join("target").join("debug").join("test-server")
    ), Capture { stdout: None, stderr: None });
    let _proxy = TemporaryChild::spawn(&mut Command::new(
        workspace_dir.join("target").join("debug").join("joining-proxy")
    ), Capture { stdout: None, stderr: None });
    sleep(Duration::from_millis(250)).await; // Wait till daemons start.
    test_calls(&test).await?;
    // TODO
    Ok(())
}