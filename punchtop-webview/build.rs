use std::path::PathBuf;
use std::process::Command;

use std::env;

fn main() {
    build_react();
}

fn build_react() {
    let html_path = "target/release/index.html";
    let html_input: PathBuf = [
        &env::var("CARGO_MANIFEST_DIR").unwrap(),
        "../punchtop-react",
        html_path,
    ]
    .iter()
    .collect();
    let html_input = html_input.to_str().unwrap();

    let ui_dir: PathBuf = [
        &env::var("CARGO_MANIFEST_DIR").unwrap(),
        "../punchtop-react",
    ]
    .iter()
    .collect();
    let ui_dir = ui_dir.to_str().unwrap();

    if Command::new("yarn")
        .current_dir(ui_dir)
        .args(&[
            "build-release",
            "--output-path",
            &env::var("OUT_DIR").unwrap(),
        ])
        .status()
        .unwrap()
        .success()
    {
        println!("cargo:rerun-if-changed=\"{}\"", html_input);
    } else {
        panic!("Failed to create elm");
    }
}
