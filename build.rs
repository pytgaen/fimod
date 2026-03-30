use std::fs;

fn main() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("Failed to read Cargo.toml");
    for line in cargo_toml.lines() {
        if line.starts_with("monty") {
            if let Some(tag) = line.split("tag = \"v").nth(1) {
                let version = tag.split('"').next().unwrap_or("").trim();
                println!("cargo:rustc-env=MONTY_VERSION={version}");
                return;
            }
        }
    }
    panic!("Could not extract monty version from Cargo.toml");
}
