use assert_fs::prelude::*;
use predicates::prelude::*;

fn sandbox_content(home: &assert_fs::TempDir) -> String {
    std::fs::read_to_string(home.path().join(".config/fimod/sandbox.toml")).unwrap()
}

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
    assert!(content.contains(r#"max_duration = "10m""#));
    assert!(content.contains(r#"max_memory   = "2GB""#));
    assert!(content.contains("allow_env    = []"));
}

/// `--preset strict` writes the stricter sandbox preset.
#[test]
fn test_setup_sandbox_defaults_strict_preset() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "setup", "sandbox", "defaults", "--yes", "--preset", "strict",
        ])
        .env("HOME", home.path())
        .assert()
        .success();

    let content = sandbox_content(&home);
    assert!(content.contains("allow_clock  = false"));
    assert!(content.contains(r#"max_duration = "30s""#));
    assert!(content.contains(r#"max_memory   = "512MB""#));
    assert!(content.contains("allow_env    = []"));
}

/// `--sandbox-file` lets defaults write an explicit policy file.
#[test]
fn test_setup_sandbox_defaults_explicit_file_permissive_preset() {
    let dir = assert_fs::TempDir::new().unwrap();
    let sandbox = dir.path().join("ci-sandbox.toml");
    let sandbox_arg = sandbox.to_string_lossy().to_string();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "setup",
            "sandbox",
            "defaults",
            "--yes",
            "--sandbox-file",
            &sandbox_arg,
            "--preset",
            "permissive",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&sandbox).unwrap();
    assert!(content.contains("allow_clock  = true"));
    assert!(content.contains(r#"max_duration = "30m""#));
    assert!(content.contains(r#"max_memory   = "4GB""#));
    assert!(content.contains(r#"allow_env    = ["LANG", "LC_*", "TZ", "USER", "HOME"]"#));
}

/// Explicit sandbox files can be relative paths in the current directory.
#[test]
fn test_setup_sandbox_defaults_relative_sandbox_file() {
    let dir = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .current_dir(dir.path())
        .args([
            "setup",
            "sandbox",
            "defaults",
            "--yes",
            "--sandbox-file",
            "ci-sandbox.toml",
        ])
        .assert()
        .success();

    assert!(dir.path().join("ci-sandbox.toml").is_file());
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

/// `setup sandbox set` creates the canonical file from the recommended preset plus overrides.
#[test]
fn test_setup_sandbox_set_creates_canonical_file() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "setup",
            "sandbox",
            "set",
            "--max-memory",
            "4GB",
            "--allow-env",
            "LANG",
            "--allow-env",
            "TZ_*",
        ])
        .env("HOME", home.path())
        .assert()
        .success();

    let content = sandbox_content(&home);
    assert!(content.contains("allow_clock  = true"));
    assert!(content.contains(r#"max_duration = "10m""#));
    assert!(content.contains(r#"max_memory   = "4GB""#));
    assert!(content.contains(r#"allow_env    = ["LANG", "TZ_*"]"#));
}

/// `setup sandbox set --sandbox-file` creates an explicit policy file.
#[test]
fn test_setup_sandbox_set_creates_explicit_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let sandbox = dir.path().join("nested/policy.toml");
    let sandbox_arg = sandbox.to_string_lossy().to_string();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "setup",
            "sandbox",
            "set",
            "--sandbox-file",
            &sandbox_arg,
            "--max-duration",
            "1m",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&sandbox).unwrap();
    assert!(content.contains(r#"max_duration = "1m""#));
    assert!(content.contains(r#"max_memory   = "2GB""#));
}

/// `setup sandbox set` preserves unrelated existing policy fields.
#[test]
fn test_setup_sandbox_set_preserves_existing_fields() {
    let home = assert_fs::TempDir::new().unwrap();
    let config_dir = home.child(".config/fimod");
    config_dir.create_dir_all().unwrap();
    config_dir
        .child("sandbox.toml")
        .write_str(
            "[sandbox]\nallow_clock = false\nmax_duration = \"1m\"\nmax_memory = \"512MB\"\nallow_env = [\"LANG\"]\n",
        )
        .unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "set", "--max-memory", "3GB"])
        .env("HOME", home.path())
        .assert()
        .success();

    let content = sandbox_content(&home);
    assert!(content.contains("allow_clock  = false"));
    assert!(content.contains(r#"max_duration = "1m""#));
    assert!(content.contains(r#"max_memory   = "3GB""#));
    assert!(content.contains(r#"allow_env    = ["LANG"]"#));
}

/// Conflicting clock flags are rejected by the CLI.
#[test]
fn test_setup_sandbox_set_rejects_clock_conflict() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "set", "--allow-clock", "--deny-clock"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

/// Conflicting env update flags are rejected by the CLI.
#[test]
fn test_setup_sandbox_set_rejects_env_conflict() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "setup",
            "sandbox",
            "set",
            "--allow-env",
            "LANG",
            "--clear-env",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

/// `show` and `get` expose normalized sandbox policy values.
#[test]
fn test_setup_sandbox_show_and_get() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "setup",
            "sandbox",
            "set",
            "--max-memory",
            "4GB",
            "--allow-env",
            "LANG,TZ",
        ])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "show"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"max_memory   = "4GB""#))
        .stdout(predicate::str::contains(r#"allow_env    = ["LANG", "TZ"]"#));

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "get", "max-memory"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout("4GB\n");

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "get", "allow-env"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout("LANG\nTZ\n");
}

/// `show` prints the recommended preset when the target file does not exist.
#[test]
fn test_setup_sandbox_show_missing_file_prints_recommended() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "show"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("allow_clock  = true"))
        .stdout(predicate::str::contains(r#"max_duration = "10m""#))
        .stdout(predicate::str::contains(r#"max_memory   = "2GB""#))
        .stdout(predicate::str::contains("allow_env    = []"));

    assert!(!home.path().join(".config/fimod/sandbox.toml").exists());
}

/// Unknown sandbox keys are rejected by clap value parsing.
#[test]
fn test_setup_sandbox_get_rejects_unknown_key() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "get", "unknown-key"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

/// Invalid setup sandbox values fail before writing.
#[test]
fn test_setup_sandbox_set_rejects_invalid_values() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "set", "--max-memory", "bad"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("--max-memory"));

    assert!(!home.path().join(".config/fimod/sandbox.toml").exists());
}

/// Invalid duration values use the duration parser context.
#[test]
fn test_setup_sandbox_set_rejects_invalid_duration() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "set", "--max-duration", "bad"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("--max-duration"));

    assert!(!home.path().join(".config/fimod/sandbox.toml").exists());
}

/// Empty `--sandbox-file` is runtime-only and rejected for setup commands.
#[test]
fn test_setup_sandbox_rejects_empty_sandbox_file() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "show", "--sandbox-file", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--sandbox-file cannot be empty"));
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

/// `setup all defaults --preset` forwards the preset to sandbox setup.
#[test]
fn test_setup_all_defaults_forwards_sandbox_preset() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "all", "defaults", "--yes", "--preset", "strict"])
        .env("HOME", home.path())
        .assert()
        .success();

    let content = sandbox_content(&home);
    assert!(content.contains(r#"max_duration = "30s""#));
    assert!(content.contains(r#"max_memory   = "512MB""#));
}

/// `--if-needed` reads setup env vars, so installers can pass answers through
/// without duplicating setup logic in shell.
#[test]
fn test_setup_all_if_needed_honors_env_yes() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "all", "defaults", "--if-needed"])
        .env("HOME", home.path())
        .env("FIMOD_SETUP_ALL", "yes")
        .assert()
        .success();

    assert!(home.path().join(".config/fimod/sandbox.toml").is_file());
    assert!(home.path().join(".config/fimod/sources.toml").is_file());
}

/// Granular env vars override FIMOD_SETUP_ALL for their own setup block.
#[test]
fn test_setup_all_if_needed_honors_granular_env_no() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "all", "defaults", "--if-needed"])
        .env("HOME", home.path())
        .env("FIMOD_SETUP_ALL", "yes")
        .env("FIMOD_SETUP_REGISTRY", "no")
        .assert()
        .success();

    assert!(home.path().join(".config/fimod/sandbox.toml").is_file());
    assert!(!home.path().join(".config/fimod/sources.toml").exists());
}

/// On upgrades, `--if-needed` leaves an existing sandbox policy untouched.
#[test]
fn test_setup_sandbox_if_needed_preserves_existing_file() {
    let home = assert_fs::TempDir::new().unwrap();
    let config_dir = home.child(".config/fimod");
    config_dir.create_dir_all().unwrap();
    config_dir
        .child("sandbox.toml")
        .write_str("# custom policy\n")
        .unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["setup", "sandbox", "defaults", "--if-needed"])
        .env("HOME", home.path())
        .env("FIMOD_SETUP_SANDBOX", "yes")
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));

    let content = std::fs::read_to_string(home.path().join(".config/fimod/sandbox.toml")).unwrap();
    assert_eq!(content, "# custom policy\n");
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
