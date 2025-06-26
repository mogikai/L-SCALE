pub fn systemdir() -> String {
    format!("{}/systems", env!("OUT_DIR"))
}
