use std::fs;

fn main() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("Failed to read Cargo.toml");
    for line in cargo_toml.lines() {
        if let Some(version) = line
            .strip_prefix("monty = \"")
            .and_then(|value| value.strip_suffix('"'))
        {
            println!("cargo:rustc-env=MONTY_VERSION={version}");
            return;
        }
    }
    panic!("Could not extract monty version from Cargo.toml");
}
