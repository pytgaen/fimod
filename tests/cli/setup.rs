use assert_fs::prelude::*;
use predicates::prelude::*;

/// `fimod setup sandbox defaults --yes` creates the canonical file with the preset.
#[test]
fn test_setup_sandbox_defaults_writes_file() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "defaults", "--yes"])
        .env("HOME", home.path())
        .env_remove("FIMOD_SANDBOX_FILE")
        .assert()
        .success();

    let config_path = home.path().join(".config/fimod/sandbox.toml");
    assert!(config_path.is_file(), "sandbox.toml was not created");
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("allow_clock  = true"));
    assert!(content.contains(r#"max_duration = "2m""#));
    assert!(content.contains(r#"max_memory   = "1GB""#));
    assert!(content.contains("allow_env    = []"));
}

/// Running `setup sandbox defaults --yes` twice fails the second time without `--force`.
#[test]
fn test_setup_sandbox_defaults_refuses_overwrite() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "defaults", "--yes"])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "defaults", "--yes"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"))
        .stderr(predicate::str::contains("--force"));
}

/// `--force` lets `setup sandbox defaults` overwrite an existing file.
#[test]
fn test_setup_sandbox_defaults_force_overwrites() {
    let home = assert_fs::TempDir::new().unwrap();

    // Pre-populate with garbage content to prove we really overwrote.
    let config_dir = home.child(".config/fimod");
    config_dir.create_dir_all().unwrap();
    config_dir
        .child("sandbox.toml")
        .write_str("# placeholder\n")
        .unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "defaults", "--yes", "--force"])
        .env("HOME", home.path())
        .assert()
        .success();

    let content = std::fs::read_to_string(home.path().join(".config/fimod/sandbox.toml")).unwrap();
    assert!(content.contains("allow_clock"));
    assert!(!content.contains("placeholder"));
}

/// Legacy `fimod registry setup` still works and prints a deprecation warning.
#[test]
fn test_registry_setup_prints_deprecation_warning() {
    let home = assert_fs::TempDir::new().unwrap();

    // --yes keeps it non-interactive; in a TTY-less context `confirm` skips prompts anyway.
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "setup", "--yes"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("deprecated"))
        .stderr(predicate::str::contains("fimod setup registry defaults"))
        .stderr(predicate::str::contains("0.10.0"));
}

/// `fimod setup registry defaults --yes` succeeds without the deprecation warning.
#[test]
fn test_setup_registry_defaults_no_warning() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "registry", "defaults", "--yes"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("deprecated").not());
}

/// `fimod setup all defaults --yes` writes the sandbox file and installs registries.
#[test]
fn test_setup_all_defaults_runs_both() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "all", "defaults", "--yes"])
        .env("HOME", home.path())
        .assert()
        .success();

    assert!(
        home.path().join(".config/fimod/sandbox.toml").is_file(),
        "sandbox.toml must exist after setup all defaults"
    );
    assert!(
        home.path().join(".config/fimod/sources.toml").is_file(),
        "sources.toml must exist after setup all defaults"
    );
}

/// `fimod setup all defaults` fails at first error: if sandbox is pre-existing without --force,
/// registry must still have been configured (runs first).
#[test]
fn test_setup_all_defaults_fails_at_first_error() {
    let home = assert_fs::TempDir::new().unwrap();
    let config_dir = home.child(".config/fimod");
    config_dir.create_dir_all().unwrap();
    config_dir
        .child("sandbox.toml")
        .write_str("# pre-existing\n")
        .unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "all", "defaults", "--yes"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    // sandbox.toml was preserved (not overwritten).
    let content = std::fs::read_to_string(home.path().join(".config/fimod/sandbox.toml")).unwrap();
    assert!(content.contains("pre-existing"));
}
