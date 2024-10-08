use std::{env, fs, process::Command};

fn main() {
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

        Command::new("glslc")
            .args(&[&infile, "-o", &outfile])
            .status()
            .unwrap();
    }

    println!("cargo::rerun-if-changed=src/shaders")
}
