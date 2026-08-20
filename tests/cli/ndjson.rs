use super::helpers::setup_input;
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn test_ndjson_input_to_json() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "ndjson",
            "--output-format",
            "json",
            "-e",
            "data",
        ])
        .write_stdin("{\"name\":\"Alice\"}\n{\"name\":\"Bob\"}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"Alice\""))
        .stdout(predicate::str::contains("\"name\": \"Bob\""));
}

#[test]
fn test_ndjson_identity_streams_file_to_pretty_json() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "data.ndjson",
        "{\"name\":\"Alice\"}\n\n{\"id\":18446744073709551615}\n",
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "data", "--output-format", "json"])
        .assert()
        .success()
        .stdout(
            "[\n  {\n    \"name\": \"Alice\"\n  },\n  {\n    \"id\": 18446744073709551615\n  }\n]\n",
        );
}

#[test]
fn test_ndjson_identity_streams_stdin_to_compact_json() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "ndjson",
            "--output-format",
            "json-compact",
            "-e",
            "data",
        ])
        .write_stdin("{\"a\":1}\n  \n{\"b\":2}\n")
        .assert()
        .success()
        .stdout("[{\"a\":1},{\"b\":2}]\n");
}

#[test]
fn test_ndjson_identity_stream_reports_invalid_line() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "ndjson",
            "--output-format",
            "json-compact",
            "-e",
            "data",
        ])
        .write_stdin("{\"a\":1}\nnot-json\n")
        .assert()
        .failure()
        .stdout(predicate::str::starts_with("[{\"a\":1}"))
        .stderr(predicate::str::contains(
            "Failed to parse NDJSON line: not-json",
        ));
}

#[test]
fn test_ndjson_identity_error_preserves_existing_output_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.ndjson", "{\"a\":1}\nnot-json\n");
    let output = dir.child("output.json");
    output.write_str("keep me").unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "data", "-o"])
        .arg(output.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Failed to parse NDJSON line: not-json",
        ));

    output.assert("keep me");
}

#[test]
fn test_ndjson_identity_replaces_existing_output_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.ndjson", "{\"a\":1}\n{\"b\":2}\n");
    let output = dir.child("output.json");
    output.write_str("old output").unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "data", "-o"])
        .arg(output.path())
        .assert()
        .success();

    output.assert("[\n  {\n    \"a\": 1\n  },\n  {\n    \"b\": 2\n  }\n]\n");
}

#[cfg(unix)]
#[test]
fn test_ndjson_identity_preserves_existing_output_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.ndjson", "{\"a\":1}\n");
    let output = dir.child("output.json");
    output.write_str("private").unwrap();
    std::fs::set_permissions(output.path(), std::fs::Permissions::from_mode(0o600)).unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "data", "-o"])
        .arg(output.path())
        .assert()
        .success();

    let mode = std::fs::metadata(output.path())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[cfg(unix)]
#[test]
fn test_ndjson_identity_follows_output_symlink() {
    use std::os::unix::fs::symlink;

    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.ndjson", "{\"a\":1}\n");
    let target = dir.child("target.json");
    let output = dir.child("output.json");
    target.write_str("old output").unwrap();
    symlink(target.path(), output.path()).unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "data", "-o"])
        .arg(output.path())
        .assert()
        .success();

    assert!(std::fs::symlink_metadata(output.path())
        .unwrap()
        .file_type()
        .is_symlink());
    target.assert("[\n  {\n    \"a\": 1\n  }\n]\n");
}

#[cfg(unix)]
#[test]
fn test_ndjson_identity_hard_link_output_does_not_destroy_input() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("input.ndjson");
    let output = dir.child("output.json");
    let original = "{\"a\":1}\n{\"b\":2}\n";
    input.write_str(original).unwrap();
    std::fs::hard_link(input.path(), output.path()).unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i"])
        .arg(input.path())
        .args(["-e", "data", "--output-format", "json-compact", "-o"])
        .arg(output.path())
        .assert()
        .success();

    input.assert(original);
    output.assert("[{\"a\":1},{\"b\":2}]\n");
}

#[test]
fn test_ndjson_identity_debug_uses_buffered_path() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "ndjson",
            "--output-format",
            "json-compact",
            "--debug",
            "-e",
            "data",
        ])
        .write_stdin("{\"a\":1}\n{\"b\":2}\n")
        .assert()
        .success()
        .stdout("[{\"a\":1},{\"b\":2}]\n")
        .stderr(predicate::str::contains("[debug] input data:"));
}

#[test]
fn test_ndjson_identity_check_does_not_stream_output() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "ndjson",
            "--output-format",
            "json-compact",
            "--check",
            "-e",
            "data",
        ])
        .write_stdin("{\"a\":1}\n{\"b\":2}\n")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_ndjson_identity_in_place_does_not_truncate_input() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.ndjson", "{\"a\":1}\n{\"b\":2}\n");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "--output-format",
            "json",
            "--in-place",
            "-e",
            "data",
        ])
        .assert()
        .success();

    dir.child("data.ndjson")
        .assert("[\n  {\n    \"a\": 1\n  },\n  {\n    \"b\": 2\n  }\n]\n");
}

#[test]
fn test_ndjson_identity_stream_matches_buffered_identity_matrix() {
    let cases = [
        ("json", "{\"a\":1}\n\n{\"b\":2}\n"),
        (
            "json-compact",
            "null\ntrue\n18446744073709551615\n1234567890123456789012345678901234567890\n\"text\"\n",
        ),
        ("json-compact", "\n  \n"),
    ];

    for (output_format, input) in cases {
        let native = assert_cmd::cargo_bin_cmd!("fimod")
            .arg("shape")
            .args([
                "--input-format",
                "ndjson",
                "--output-format",
                output_format,
                "-e",
                "data",
            ])
            .write_stdin(input)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let buffered = assert_cmd::cargo_bin_cmd!("fimod")
            .arg("shape")
            .args([
                "--input-format",
                "ndjson",
                "--output-format",
                output_format,
                "-e",
                "data",
                "--debug",
            ])
            .write_stdin(input)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        assert_eq!(native, buffered, "parity failed for {output_format}");
    }
}

#[test]
fn test_ndjson_output() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[{"name": "Alice"}, {"name": "Bob"}]"#);

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "--output-format", "ndjson", "-e", "data"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = stdout.trim_end().split('\n').collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"name\":\"Alice\""));
    assert!(lines[1].contains("\"name\":\"Bob\""));
}

#[test]
fn test_json_array_to_ndjson_identity_streams_to_output_extension() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "pretty.json",
        r#"[
  {
    "name": "Alice"
  },
  {
    "name": "Bob"
  }
]"#,
    );
    let output = dir.child("out.jsonl");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "data", "-o"])
        .arg(output.path())
        .assert()
        .success();

    output.assert("{\"name\":\"Alice\"}\n{\"name\":\"Bob\"}\n");
}

#[test]
fn test_json_array_to_ndjson_identity_streams_from_stdin() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "json",
            "--output-format",
            "ndjson",
            "-e",
            "data",
        ])
        .write_stdin("[\n  {\"a\": 1},\n  {\"b\": 2}\n]\n")
        .assert()
        .success()
        .stdout("{\"a\":1}\n{\"b\":2}\n");
}

#[test]
fn test_json_object_to_ndjson_identity_still_outputs_single_line() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "json",
            "--output-format",
            "ndjson",
            "-e",
            "data",
        ])
        .write_stdin("{\"name\":\"Alice\"}\n")
        .assert()
        .success()
        .stdout("{\"name\":\"Alice\"}\n");
}

#[test]
fn test_json_array_to_ndjson_identity_in_place_does_not_truncate_input() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[{"a":1},{"b":2}]"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "--output-format",
            "ndjson",
            "--in-place",
            "-e",
            "data",
        ])
        .assert()
        .success();

    dir.child("data.json").assert("{\"a\":1}\n{\"b\":2}\n");
}

#[test]
fn test_identity_expression_converts_without_monty_step() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[{"name": "Alice"}, {"name": "Bob"}]"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "--output-format",
            "ndjson",
            "--debug",
            "-e",
            "data",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("{\"name\":\"Alice\"}"))
        .stdout(predicate::str::contains("{\"name\":\"Bob\"}"))
        .stderr(predicate::str::contains("step 1/1").not());
}

#[test]
fn test_ndjson_file_extension_detection() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.ndjson", "{\"a\":1}\n{\"b\":2}\n");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "len(data)", "--output-format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2"));
}

#[test]
fn test_ndjson_len_expression() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--input-format", "ndjson", "-e", "len(data)"])
        .write_stdin("{\"name\":\"Alice\"}\n{\"name\":\"Bob\"}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("2"));
}

#[test]
fn test_ndjson_roundtrip() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "ndjson",
            "--output-format",
            "ndjson",
            "-e",
            "data",
        ])
        .write_stdin("{\"a\":1}\n{\"b\":2}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("{\"a\":1}"))
        .stdout(predicate::str::contains("{\"b\":2}"));
}

#[test]
fn test_slurp_multiple_json() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--input-format", "json", "-s", "-e", "len(data)"])
        .write_stdin("{\"a\":1}\n{\"b\":2}")
        .assert()
        .success()
        .stdout(predicate::str::contains("2"));
}

#[test]
fn test_slurp_single_json() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--input-format", "json", "-s", "-e", "len(data)"])
        .write_stdin("{\"a\":1}")
        .assert()
        .success()
        .stdout(predicate::str::contains("1"));
}

#[test]
fn test_slurp_ndjson() {
    // Slurp + NDJSON: NDJSON already produces an array, slurp is a no-op
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--input-format", "ndjson", "-s", "-e", "len(data)"])
        .write_stdin("{\"a\":1}\n{\"b\":2}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("2"));
}

#[test]
fn test_slurp_count() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--input-format", "json", "-s", "-e", "len(data)"])
        .write_stdin("{\"a\":1}{\"b\":2}{\"c\":3}")
        .assert()
        .success()
        .stdout(predicate::str::contains("3"));
}

#[test]
fn test_slurp_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "multi.json", "{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-s", "-e", "len(data)"])
        .assert()
        .success()
        .stdout(predicate::str::contains("3"));
}

#[test]
fn test_slurp_with_expression() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "json",
            "-s",
            "-e",
            "[d for d in data if \"a\" in d]",
        ])
        .write_stdin("{\"a\":1}\n{\"b\":2}\n{\"a\":3}")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"a\": 1"))
        .stdout(predicate::str::contains("\"a\": 3"));
}
