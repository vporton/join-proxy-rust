use std::process::Command;

use like_shell::run_successful_command;

// use like_shell::temp_dir_from_template;
// use tempdir::TempDir;

// struct Test {
//     // dir: TempDir,
// }

// impl Test {
//     pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
//         Ok(Self {
//             // dir: temp_dir_from_template(tmpl_dir)?,
//         })
//     }
// }

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let cargo_manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    // let workspace_dir = cargo_manifest_dir.join("..").join("..");
    // let tmpl_dir = .join("tmpls");
    // let test = Test::new()?;
    run_successful_command(Command::new("dfx").args(["deploy"]))?;
    // TODO
    Ok(())
}