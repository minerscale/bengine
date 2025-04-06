use std::{env, fs, process::Command};

fn main() -> anyhow::Result<()> {
    let paths = fs::read_dir("src/renderer/shaders/")?;

    let out_dir = env::var("OUT_DIR")?;

    for path in paths {
        let path = path?;
        if path.file_type()?.is_dir() {
            unimplemented!("nested directories not yet supported")
        }

        let path = &path;
        let in_path = path.path();
        let infile = in_path.to_string_lossy();

        let outfile = out_dir.clone() + "/" + path.file_name().to_str().unwrap() + ".spv";

        let output = Command::new("glslc")
            .args([&infile, "-o", &outfile])
            .output()?;

        if !output.status.success() {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "failed to compile {}\n\n{}",
                    path.file_name().to_string_lossy(),
                    std::str::from_utf8(&output.stderr)?
                ),
            ))?;
        }

        println!("cargo::rerun-if-changed=src/shaders/{infile}")
    }

    Ok(())
}
