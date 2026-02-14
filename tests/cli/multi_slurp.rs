use super::helpers::setup_input;
use assert_fs::prelude::*;
use predicates::prelude::*;

// ── List mode ─────────────────────────────────────────────────────────────────

#[test]
fn test_multi_slurp_list_two_json() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "a.json", r#"{"x": 1}"#);
    let f2 = setup_input(&dir, "b.json", r#"{"x": 2}"#);

    // data should be [{"x":1}, {"x":2}]; sum of x values = 3
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &f1,
            "-i",
            &f2,
            "-s",
            "-e",
            "sum(d['x'] for d in data)",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("3"));
}

#[test]
fn test_multi_slurp_list_access_by_index() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "first.json", r#"{"name": "alice"}"#);
    let f2 = setup_input(&dir, "second.json", r#"{"name": "bob"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &f1, "-i", &f2, "-s", "-e", "data[1]['name']"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bob"));
}

#[test]
fn test_multi_slurp_list_cross_format() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "config.json", r#"{"env": "prod"}"#);
    let f2 = setup_input(&dir, "override.yaml", "env: staging\nextra: true\n");

    // Both files are accessible: JSON at index 0, YAML at index 1
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &f1,
            "-i",
            &f2,
            "-s",
            "-e",
            "[data[0]['env'], data[1]['env']]",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("prod"))
        .stdout(predicate::str::contains("staging"));
}

#[test]
fn test_multi_slurp_list_three_files() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "n1.json", "1");
    let f2 = setup_input(&dir, "n2.json", "2");
    let f3 = setup_input(&dir, "n3.json", "3");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &f1, "-i", &f2, "-i", &f3, "-s", "-e", "sum(data)"])
        .assert()
        .success()
        .stdout(predicate::str::contains("6"));
}

// ── Named mode (auto stem) ────────────────────────────────────────────────────

#[test]
fn test_multi_slurp_named_stem_access() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "base.json", r#"{"color": "red"}"#);
    let f2 = setup_input(&dir, "overlay.json", r#"{"color": "blue", "size": 10}"#);

    // Keys: "base" and "overlay" (stems without extension)
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &format!("{f1}:"),
            "-i",
            &format!("{f2}:"),
            "-s",
            "-e",
            "data['overlay']['color']",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("blue"));
}

#[test]
fn test_multi_slurp_named_stem_merge() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "defaults.json", r#"{"timeout": 30, "retries": 3}"#);
    let f2 = setup_input(&dir, "prod.json", r#"{"timeout": 60}"#);

    // Both dicts are accessible by stem key
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &format!("{f1}:"),
            "-i",
            &format!("{f2}:"),
            "-s",
            "-e",
            "[data['defaults']['retries'], data['prod']['timeout']]",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("3"))
        .stdout(predicate::str::contains("60"));
}

// ── Named mode (explicit alias) ───────────────────────────────────────────────

#[test]
fn test_multi_slurp_named_explicit_alias() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "x.json", r#"{"v": 100}"#);
    let f2 = setup_input(&dir, "y.json", r#"{"v": 200}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &format!("{f1}:old"),
            "-i",
            &format!("{f2}:new"),
            "-s",
            "-e",
            "data['new']['v'] - data['old']['v']",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("100"));
}

#[test]
fn test_multi_slurp_named_explicit_resolves_stem_collision() {
    // Two files named "base" in different dirs — use explicit aliases to disambiguate
    let dir = assert_fs::TempDir::new().unwrap();
    let sub1 = dir.child("sub1");
    sub1.create_dir_all().unwrap();
    let sub2 = dir.child("sub2");
    sub2.create_dir_all().unwrap();

    let f1 = sub1.child("base.json");
    f1.write_str(r#"{"src": "sub1"}"#).unwrap();
    let f2 = sub2.child("base.json");
    f2.write_str(r#"{"src": "sub2"}"#).unwrap();

    let p1 = f1.path().to_str().unwrap().to_string();
    let p2 = f2.path().to_str().unwrap().to_string();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &format!("{p1}:first"),
            "-i",
            &format!("{p2}:second"),
            "-s",
            "-e",
            "[data['first']['src'], data['second']['src']]",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("sub1"))
        .stdout(predicate::str::contains("sub2"));
}

// ── Output to file ────────────────────────────────────────────────────────────

#[test]
fn test_multi_slurp_output_to_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "p1.json", r#"{"n": 1}"#);
    let f2 = setup_input(&dir, "p2.json", r#"{"n": 2}"#);
    let out = dir.child("result.json");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &f1,
            "-i",
            &f2,
            "-s",
            "-e",
            "{'total': sum(d['n'] for d in data)}",
            "-o",
            out.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    out.assert(predicate::path::exists());
    let content = std::fs::read_to_string(out.path()).unwrap();
    assert!(
        content.contains("total") && content.contains("3"),
        "expected total=3, got: {content}"
    );
}

// ── Error cases ───────────────────────────────────────────────────────────────

#[test]
fn test_multi_slurp_error_mix_alias_and_plain() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "a.json", r#"{"x": 1}"#);
    let f2 = setup_input(&dir, "b.json", r#"{"x": 2}"#);

    // f1 has ':', f2 doesn't → error
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &format!("{f1}:foo"), "-i", &f2, "-s", "-e", "data"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot mix"));
}

#[test]
fn test_multi_slurp_error_in_place() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "a.json", r#"{"x": 1}"#);
    let f2 = setup_input(&dir, "b.json", r#"{"x": 2}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &f1, "-i", &f2, "-s", "--in-place", "-e", "data"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("incompatible with --in-place"));
}

#[test]
fn test_multi_slurp_error_output_is_directory() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "a.json", r#"{"x": 1}"#);
    let f2 = setup_input(&dir, "b.json", r#"{"x": 2}"#);
    let out_dir = dir.child("outdir");
    out_dir.create_dir_all().unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &f1,
            "-i",
            &f2,
            "-s",
            "-e",
            "data",
            "-o",
            out_dir.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("must be a file"));
}

#[test]
fn test_multi_slurp_error_duplicate_stem() {
    // Two files with the same stem in named mode → duplicate key error
    let dir = assert_fs::TempDir::new().unwrap();
    let sub1 = dir.child("d1");
    sub1.create_dir_all().unwrap();
    let sub2 = dir.child("d2");
    sub2.create_dir_all().unwrap();

    let f1 = sub1.child("data.json");
    f1.write_str(r#"{"v": 1}"#).unwrap();
    let f2 = sub2.child("data.json");
    f2.write_str(r#"{"v": 2}"#).unwrap();

    let p1 = f1.path().to_str().unwrap().to_string();
    let p2 = f2.path().to_str().unwrap().to_string();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &format!("{p1}:"),
            "-i",
            &format!("{p2}:"),
            "-s",
            "-e",
            "data",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate key"));
}

// ── Backward compatibility: regular batch (no -s) still works ─────────────────

#[test]
fn test_batch_without_slurp_unchanged() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "u1.json", r#"{"val": "hello"}"#);
    let f2 = setup_input(&dir, "u2.json", r#"{"val": "world"}"#);
    let out_dir = dir.child("batchout");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &f1,
            "-i",
            &f2,
            "-e",
            r#"{"val": data["val"].upper()}"#,
            "-o",
            out_dir.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let o1 = out_dir.child("u1.json");
    let o2 = out_dir.child("u2.json");
    let c1 = std::fs::read_to_string(o1.path()).unwrap();
    let c2 = std::fs::read_to_string(o2.path()).unwrap();
    assert!(c1.contains("HELLO"), "got: {c1}");
    assert!(c2.contains("WORLD"), "got: {c2}");
}
