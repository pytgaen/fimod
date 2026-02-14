use assert_fs::prelude::*;
use predicates::prelude::*;

// ── FIMOD_REGISTRY basic resolution ─────────────────────────────────────────

#[test]
fn test_fimod_registry_env_local() {
    let home = assert_fs::TempDir::new().unwrap();
    let molds_dir = assert_fs::TempDir::new().unwrap();
    molds_dir
        .child("normalize.py")
        .write_str("def transform(data, args, env, headers):\n    return {'normalized': True}\n")
        .unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@normalize"])
        .env("HOME", home.path())
        .env("FIMOD_REGISTRY", molds_dir.path().to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("normalized"));
}

#[test]
fn test_fimod_registry_env_no_sources_toml_needed() {
    let home = assert_fs::TempDir::new().unwrap();
    let molds_dir = assert_fs::TempDir::new().unwrap();
    molds_dir
        .child("identity.py")
        .write_str("def transform(data, args, env, headers):\n    return data\n")
        .unwrap();

    // No registry add — only FIMOD_REGISTRY
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@identity"])
        .env("HOME", home.path())
        .env("FIMOD_REGISTRY", molds_dir.path().to_str().unwrap())
        .assert()
        .success();
}

// ── multiple registries (comma-separated) ───────────────────────────────────

#[test]
fn test_fimod_registry_env_multiple_comma_separated() {
    let home = assert_fs::TempDir::new().unwrap();
    let dir1 = assert_fs::TempDir::new().unwrap();
    let dir2 = assert_fs::TempDir::new().unwrap();

    // mold_a only in dir1
    dir1.child("mold_a.py")
        .write_str("def transform(data, args, env, headers):\n    return {'from': 'dir1'}\n")
        .unwrap();

    // mold_b only in dir2
    dir2.child("mold_b.py")
        .write_str("def transform(data, args, env, headers):\n    return {'from': 'dir2'}\n")
        .unwrap();

    let env_val = format!(
        "{},{}",
        dir1.path().to_str().unwrap(),
        dir2.path().to_str().unwrap()
    );

    // Resolve mold_a from dir1
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@mold_a"])
        .env("HOME", home.path())
        .env("FIMOD_REGISTRY", &env_val)
        .assert()
        .success()
        .stdout(predicate::str::contains("dir1"));

    // Resolve mold_b from dir2
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@mold_b"])
        .env("HOME", home.path())
        .env("FIMOD_REGISTRY", &env_val)
        .assert()
        .success()
        .stdout(predicate::str::contains("dir2"));
}

// ── FIMOD_REGISTRY takes priority over sources.toml ─────────────────────────

#[test]
fn test_fimod_registry_priority_over_sources_toml() {
    let home = assert_fs::TempDir::new().unwrap();
    let toml_dir = assert_fs::TempDir::new().unwrap();
    let env_dir = assert_fs::TempDir::new().unwrap();

    // Same mold name in both, different output
    toml_dir
        .child("check.py")
        .write_str("def transform(data, args, env, headers):\n    return {'source': 'toml'}\n")
        .unwrap();
    env_dir
        .child("check.py")
        .write_str("def transform(data, args, env, headers):\n    return {'source': 'env'}\n")
        .unwrap();

    // Register toml_dir as default in sources.toml
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "main", toml_dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    // FIMOD_REGISTRY should win (env overrides config)
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@check"])
        .env("HOME", home.path())
        .env("FIMOD_REGISTRY", env_dir.path().to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("env"));
}

// ── sources.toml as fallback when FIMOD_REGISTRY misses the mold ────────────

#[test]
fn test_sources_toml_fallback_when_env_misses() {
    let home = assert_fs::TempDir::new().unwrap();
    let toml_dir = assert_fs::TempDir::new().unwrap();
    let env_dir = assert_fs::TempDir::new().unwrap();

    // Only in toml_dir
    toml_dir
        .child("extra.py")
        .write_str("def transform(data, args, env, headers):\n    return {'found': 'toml'}\n")
        .unwrap();

    // Register toml_dir as default
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "main", toml_dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    // @extra not in env_dir (empty), falls back to sources.toml
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@extra"])
        .env("HOME", home.path())
        .env("FIMOD_REGISTRY", env_dir.path().to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("toml"));
}

// ── mold not found anywhere ─────────────────────────────────────────────────

#[test]
fn test_fimod_registry_mold_not_found() {
    let home = assert_fs::TempDir::new().unwrap();
    let molds_dir = assert_fs::TempDir::new().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@nonexistent"])
        .env("HOME", home.path())
        .env("FIMOD_REGISTRY", molds_dir.path().to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// ── named FIMOD_REGISTRY entries support @registry/mold syntax ───────────────

#[test]
fn test_fimod_registry_named_entry() {
    let home = assert_fs::TempDir::new().unwrap();
    let molds_dir = assert_fs::TempDir::new().unwrap();
    molds_dir
        .child("clean.py")
        .write_str("def transform(data, args, env, headers):\n    return {'from': 'ci'}\n")
        .unwrap();

    let env_val = format!("ci={}", molds_dir.path().to_str().unwrap());

    // @ci/clean resolves via named FIMOD_REGISTRY entry
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@ci/clean"])
        .env("HOME", home.path())
        .env("FIMOD_REGISTRY", &env_val)
        .assert()
        .success()
        .stdout(predicate::str::contains("ci"));
}

#[test]
fn test_fimod_registry_named_priority_over_sources_toml() {
    let home = assert_fs::TempDir::new().unwrap();
    let toml_dir = assert_fs::TempDir::new().unwrap();
    let env_dir = assert_fs::TempDir::new().unwrap();

    // Same registry name "main", same mold name, different output
    toml_dir
        .child("check.py")
        .write_str("def transform(data, args, env, headers):\n    return {'source': 'toml'}\n")
        .unwrap();
    env_dir
        .child("check.py")
        .write_str("def transform(data, args, env, headers):\n    return {'source': 'env'}\n")
        .unwrap();

    // Register toml_dir as "main" in sources.toml
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["registry", "add", "main", toml_dir.path().to_str().unwrap()])
        .env("HOME", home.path())
        .assert()
        .success();

    // FIMOD_REGISTRY named "main" should win over sources.toml "main"
    let env_val = format!("main={}", env_dir.path().to_str().unwrap());
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@main/check"])
        .env("HOME", home.path())
        .env("FIMOD_REGISTRY", &env_val)
        .assert()
        .success()
        .stdout(predicate::str::contains("env"));
}

#[test]
fn test_fimod_registry_unknown_named_falls_to_sources_toml() {
    let home = assert_fs::TempDir::new().unwrap();
    let env_dir = assert_fs::TempDir::new().unwrap();
    env_dir
        .child("mold.py")
        .write_str("def transform(data, args, env, headers):\n    return data\n")
        .unwrap();

    // anonymous FIMOD_REGISTRY — @ghost/mold should fail (no named "ghost")
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", "@ghost/mold"])
        .env("HOME", home.path())
        .env("FIMOD_REGISTRY", env_dir.path().to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Registry 'ghost' not found"));
}
