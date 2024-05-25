use std::path::Path;

use like_shell::temp_dir_from_template;
use tempdir::TempDir;

struct Test {
    dir: TempDir,
}

impl Test {
    pub fn new(tmpl_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            dir: temp_dir_from_template(tmpl_dir)?,
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tmpl_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmpls");
    let test = Test::new(&tmpl_dir)?;
    println!("{:?}", &test.dir);
    Ok(())
}