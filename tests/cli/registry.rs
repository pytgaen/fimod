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
        .stdout(predicate::str::contains("Added registry 'my'"))
        .stdout(predicate::str::contains("Set 'my' as default registry"));

    // List
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "list"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("my"))
        .stdout(predicate::str::contains("(default)"));
}

#[test]
fn test_registry_add_first_becomes_default_automatically() {
    let home = assert_fs::TempDir::new().unwrap();
    let molds_dir = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "registry",
            "add",
            "first",
            molds_dir.path().to_str().unwrap(),
        ])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Set 'first' as default registry"));
}

#[test]
fn test_registry_add_second_does_not_override_default() {
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
        .success()
        .stdout(predicate::str::contains("Added registry 'second'"));

    // first should still be default
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "list"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("first").and(predicate::str::contains("(default)")));
}

#[test]
fn test_registry_add_with_default_flag() {
    let home = assert_fs::TempDir::new().unwrap();
    let dir1 = assert_fs::TempDir::new().unwrap();
    let dir2 = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "first", dir1.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "registry",
            "add",
            "second",
            dir2.path().to_str().unwrap(),
            "--default",
        ])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Set 'second' as default registry"));
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
fn test_registry_set_default() {
    let home = assert_fs::TempDir::new().unwrap();
    let dir1 = assert_fs::TempDir::new().unwrap();
    let dir2 = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "alpha", dir1.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "beta", dir2.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "set-default", "beta"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Set 'beta' as default registry"));

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "list"])
        .env("HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("beta").and(predicate::str::contains("(default)")));
}

#[test]
fn test_registry_set_default_not_found() {
    let home = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "set-default", "ghost"])
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
        .stderr(predicate::str::contains("No default registry configured"));
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
