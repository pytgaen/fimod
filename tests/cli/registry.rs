use assert_fs::prelude::*;
use predicates::prelude::*;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Create a temp dir containing a valid mold script at `cleanup.py`.
fn setup_mold_dir() -> (assert_fs::TempDir, assert_fs::fixture::ChildPath) {
    let dir = assert_fs::TempDir::new().unwrap();
    let script = dir.child("cleanup.py");
    script
        .write_str("def transform(data, args, env, headers):\n    return data\n")
        .unwrap();
    (dir, script)
}

// ── registry list ─────────────────────────────────────────────────────────────

#[test]
fn test_registry_list_empty() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "list"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No registries configured"));
}

// ── registry add (local) ──────────────────────────────────────────────────────

#[test]
fn test_registry_add_local_and_list() {
    let home = assert_fs::TempDir::new().unwrap();
    let molds_dir = assert_fs::TempDir::new().unwrap();
    molds_dir
        .child("cleanup.py")
        .write_str("def transform(data, args, env, headers):\n    return data\n")
        .unwrap();

    // Add
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "my", molds_dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Added registry 'my'"));

    // List
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "list"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("my"));
}

#[test]
fn test_registry_set_priority_makes_p0() {
    let home = assert_fs::TempDir::new().unwrap();
    let dir1 = assert_fs::TempDir::new().unwrap();
    let dir2 = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "first", dir1.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "second", dir2.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "set-priority", "second", "0"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Set 'second' to P0"));

    // second should be P0 in the list
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "list"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("P0"));
}

#[test]
fn test_registry_add_duplicate_fails() {
    let home = assert_fs::TempDir::new().unwrap();
    let dir = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "my", dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "my", dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_registry_add_duplicate_path_different_name_fails() {
    let home = assert_fs::TempDir::new().unwrap();
    let dir = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "first", dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "second", dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already registered as 'first'"));
}

#[test]
fn test_registry_add_nonexistent_path_fails() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "bad", "/tmp/nonexistent_fimod_test_dir"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Path not found"));
}

#[test]
fn test_registry_add_file_not_dir_fails() {
    let home = assert_fs::TempDir::new().unwrap();
    let (_dir, script) = setup_mold_dir(); // _dir must stay alive to keep the temp dir on disk

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "bad", script.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("must be a directory"));
}

// ── registry show ─────────────────────────────────────────────────────────────

#[test]
fn test_registry_show() {
    let home = assert_fs::TempDir::new().unwrap();
    let molds_dir = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "registry",
            "add",
            "myshow",
            molds_dir.path().to_str().unwrap(),
        ])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "show", "myshow"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Name:    myshow"))
        .stdout(predicate::str::contains("Type:    local"))
        .stdout(predicate::str::contains("Exists:  yes"));
}

#[test]
fn test_registry_show_not_found() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "show", "nonexistent"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// ── registry remove ───────────────────────────────────────────────────────────

#[test]
fn test_registry_remove() {
    let home = assert_fs::TempDir::new().unwrap();
    let dir = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "removeme", dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "remove", "removeme"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed registry 'removeme'"));

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "list"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No registries configured"));
}

#[test]
fn test_registry_remove_not_found() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "remove", "nonexistent"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// ── registry set-default ──────────────────────────────────────────────────────

#[test]
fn test_registry_set_priority_not_found() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "set-priority", "ghost", "1"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// ── @source/mold lookup via -m ────────────────────────────────────────────────

#[test]
fn test_at_mold_default_registry() {
    let home = assert_fs::TempDir::new().unwrap();
    let molds_dir = assert_fs::TempDir::new().unwrap();
    molds_dir
        .child("cleanup.py")
        .write_str("def transform(data, args, env, headers):\n    return data\n")
        .unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "my", molds_dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@cleanup"])
        .env("HOME", home.path())
        .assert()
        .success();
}

#[test]
fn test_at_mold_named_registry() {
    let home = assert_fs::TempDir::new().unwrap();
    let molds_dir = assert_fs::TempDir::new().unwrap();
    molds_dir
        .child("toto.py")
        .write_str("def transform(data, args, env, headers):\n    return data\n")
        .unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "my", molds_dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@my/toto"])
        .env("HOME", home.path())
        .assert()
        .success();
}

#[test]
fn test_at_mold_not_found_in_registry() {
    let home = assert_fs::TempDir::new().unwrap();
    let molds_dir = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "my", molds_dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@my/nonexistent"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found in registry"));
}

#[test]
fn test_at_mold_no_default_registry_fails() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@cleanup"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No registry configured"));
}

#[test]
fn test_at_mold_unknown_registry_fails() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@ghost/cleanup"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Registry 'ghost' not found"));
}

// ── build-catalog ────────────────────────────────────────────────────────────

#[test]
fn test_build_catalog_direct_path() {
    let home = assert_fs::TempDir::new().unwrap();
    let molds_dir = assert_fs::TempDir::new().unwrap();
    let mold_subdir = molds_dir.child("cleanup");
    mold_subdir.create_dir_all().unwrap();
    mold_subdir
        .child("cleanup.py")
        .write_str("\"\"\"Clean up data.\"\"\"\ndef transform(data, **_):\n    return data\n")
        .unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "registry",
            "build-catalog",
            molds_dir.path().to_str().unwrap(),
        ])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Scanned 1 molds"))
        .stdout(predicate::str::contains("cleanup"));

    // catalog.toml should exist in the molds directory
    let catalog = std::fs::read_to_string(molds_dir.path().join("catalog.toml")).unwrap();
    assert!(catalog.contains("[molds.cleanup]"));
}

#[test]
fn test_build_catalog_nonexistent_path() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "build-catalog", "/nonexistent/path"])
        .env("HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a directory"));
}

#[test]
fn test_build_catalog_registry_flag() {
    let home = assert_fs::TempDir::new().unwrap();
    let (molds_dir, _script) = setup_mold_dir();

    // Register first
    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "registry",
            "add",
            "local",
            molds_dir.path().to_str().unwrap(),
        ])
        .env("HOME", home.path())
        .assert()
        .success();

    // Build catalog via --registry
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "build-catalog", "--registry", "local"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Scanned 1 molds"));
}
