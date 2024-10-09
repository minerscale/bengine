use core::str;
use std::{env, fs, process::Command};

fn main() -> anyhow::Result<()> {
    let paths = fs::read_dir("src/shaders/").unwrap();

    let out_dir = env::var("OUT_DIR").unwrap();

    for path in paths {
        if path.as_ref().unwrap().file_type().unwrap().is_dir() {
            unimplemented!("nested directories not yet supported")
        }

        let path = &path.unwrap();
        let in_path = path.path();
        let infile = in_path.to_string_lossy();

        let outfile = out_dir.clone() + "/" + path.file_name().to_str().unwrap() + ".spv";

        let output = Command::new("glslc")
            .args(&[&infile, "-o", &outfile])
            .output()?;

        if !output.status.success() {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "failed to compile {}\n\n{}",
                    path.file_name().to_string_lossy(),
                    str::from_utf8(&output.stderr).unwrap()
                ),
            ))?;
        }

        println!("cargo::rerun-if-changed=src/shaders/{infile}")
    }

    Ok(())
}
