use std::path::Path;
use std::process::Command;
use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=assets/*");
    println!("cargo:rerun-if-changed=create-system.sh");
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let out_dir = env::var_os("OUT_DIR").unwrap().into_string().unwrap();
    let system_path = format!("{}/systems/", out_dir);

    if !Path::new(&system_path).exists() {
        fs::create_dir(&system_path).unwrap();
    }
    let pwd = env::current_dir().unwrap();
    env::set_current_dir(&system_path).unwrap();
    let ret = Command::new(format!("{}/create-system.sh", manifest_dir))
        .status()
        .unwrap();
    env::set_current_dir(pwd).unwrap();
    assert!(ret.success());
}
