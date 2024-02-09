pub fn systemdir() -> String {
    return format!("{}/systems", env!("OUT_DIR"));
}
