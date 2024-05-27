use std::process::Command;

use candid::Encode;
use ic_agent::{export::Principal, Agent};
use like_shell::run_successful_command;
use dotenv::{dotenv, var};

// use like_shell::temp_dir_from_template;
// use tempdir::TempDir;

struct Test {
    // dir: TempDir,
    agent: Agent,
}

impl Test {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let res = Self {
            // dir: temp_dir_from_template(tmpl_dir)?,
            agent: Agent::builder().with_url("http://localhost:8000").build()?,
        };
        res.agent.fetch_root_key().await?; // DON'T USE this on mainnet
        Ok(res)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let cargo_manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    dotenv().ok();

    let test_canister_id = Principal::from_text(var("CANISTER_ID_TEST")?)?;

    // let workspace_dir = cargo_manifest_dir.join("..").join("..");
    // let tmpl_dir = .join("tmpls");
    let test = Test::new().await?;
    run_successful_command(Command::new("dfx").args(["deploy"]))?;
    let res = test.agent.update(&test_canister_id, "test").with_arg(Encode!(&false)?).call_and_wait().await;
    // TODO
    Ok(())
}