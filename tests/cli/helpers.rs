use assert_fs::prelude::*;

/// Helper: create a mold script that adds a greeting field
pub const GREET_MOLD: &str = r#"
def transform(data, args, env, headers):
    data["greeting"] = f"Hello {data['name']}"
    return data
"#;

/// Helper: create a mold for CSV arrays (adds greeting to each row)
pub const CSV_GREET_MOLD: &str = r#"
def transform(data, args, env, headers):
    for row in data:
        row["greeting"] = f"Hello {row['name']}"
    return data
"#;

/// Helper: create a mold that uppercases content (TXT now passes raw string)
pub const UPPER_MOLD: &str = r#"
def transform(data, args, env, headers):
    return data.strip().upper()
"#;

pub fn setup_mold(dir: &assert_fs::TempDir, name: &str, content: &str) -> String {
    let file = dir.child(name);
    file.write_str(content).unwrap();
    file.path().to_str().unwrap().to_string()
}

pub fn setup_input(dir: &assert_fs::TempDir, name: &str, content: &str) -> String {
    let file = dir.child(name);
    file.write_str(content).unwrap();
    file.path().to_str().unwrap().to_string()
}
