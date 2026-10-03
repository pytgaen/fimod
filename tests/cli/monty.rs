use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn test_monty_repl_help_mentions_sandbox_file() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["monty", "repl", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--sandbox-file <PATH>"));
}

#[test]
fn test_monty_repl_empty_sandbox_file_denies_clock() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["monty", "repl", "--sandbox-file="])
        .write_stdin("from datetime import datetime\ndatetime.now()\n")
        .assert()
        .success()
        .stderr(predicate::str::contains("PermissionError"))
        .stderr(predicate::str::contains(
            "datetime.now() denied by sandbox policy",
        ));
}

#[test]
fn test_monty_repl_sandbox_file_allows_clock() {
    let dir = assert_fs::TempDir::new().unwrap();
    let policy = dir.child("sandbox.toml");
    policy
        .write_str("[sandbox]\nallow_clock = true\nmax_duration = \"10s\"\n")
        .unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "monty",
            "repl",
            "--sandbox-file",
            policy.path().to_str().unwrap(),
        ])
        .write_stdin("from datetime import datetime\ndatetime.now() is not None\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("True"));
}

#[test]
fn test_monty_023_new_modules_and_formatting() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = super::helpers::setup_mold(
        &dir,
        "stdlib.py",
        r#"
import base64
from functools import reduce, partial
from itertools import accumulate, batched, zip_longest
from datetime import time

def transform(data, **_):
    return {
        "encoded": base64.b64encode(b"fimod").decode(),
        "sum": reduce(lambda a, b: a + b, [1, 2, 3]),
        "partial": partial(pow, 2)(3),
        "accumulate": list(accumulate([1, 2, 3])),
        "batched": list(batched([1, 2, 3], 2)),
        "zip": list(zip_longest([1, 2], [3])),
        "format": "{:04d}".format(23),
        "time": time(14, 30, 1, 42),
    }
"#,
    );
    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "s",
            "--no-input",
            "-m",
            &mold,
            "--output-format",
            "json-compact",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let result: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        result,
        serde_json::json!({"encoded":"Zmltb2Q=", "sum":6,
        "partial":8, "accumulate":[1,3,6], "batched":[[1,2],[3]],
        "zip":[[1,3],[2,null]], "format":"0023", "time":"14:30:01.000042"})
    );
}

#[test]
fn test_monty_repl_continues_decorators_and_triple_quoted_strings() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["monty", "repl", "--sandbox-file="])
        .write_stdin("from dataclasses import dataclass\n@dataclass\nclass Point:\n    x: int\n\nPoint(23).x\ntext = \"\"\"hello\nworld\"\"\"\nlen(text)\n")
        .assert().success().stderr("")
        .stdout(predicate::str::contains("23")).stdout(predicate::str::contains("11"));
}

#[test]
fn test_monty_graph_shared_values_and_cycles_at_json_boundary() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = dir.child("graph.py");
    mold.write_str(
        "def transform(data, **_):\n    row = {'n': 18446744073709551615}\n    return [row, row]\n",
    )
    .unwrap();
    for format in ["json", "json-compact"] {
        let output = assert_cmd::cargo_bin_cmd!("fimod")
            .args([
                "s",
                "--no-input",
                "-m",
                mold.path().to_str().unwrap(),
                "--output-format",
                format,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(value, serde_json::json!([{"n": u64::MAX}, {"n": u64::MAX}]));
    }
    mold.write_str(
        "def transform(data, **_):\n    items = []\n    items.append(items)\n    return items\n",
    )
    .unwrap();
    for format in ["json", "json-compact"] {
        assert_cmd::cargo_bin_cmd!("fimod")
            .args([
                "s",
                "--no-input",
                "-m",
                mold.path().to_str().unwrap(),
                "--output-format",
                format,
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "Cannot convert MontyObject variant to JSON",
            ));
    }
}

#[test]
fn test_monty_class_names_do_not_impersonate_native_containers() {
    let dir = assert_fs::TempDir::new().unwrap();
    for (name, helper) in [
        ("list", "it_sort_by(value, 'x')"),
        ("dict", "it_keys(value)"),
    ] {
        let mold = dir.child("named_class.py");
        mold.write_str(&format!("class {name}:\n    pass\ndef transform(data, **_):\n    value = {name}()\n    return {helper}\n")).unwrap();
        assert_cmd::cargo_bin_cmd!("fimod")
            .args(["s", "--no-input", "-m", mold.path().to_str().unwrap()])
            .assert()
            .code(1)
            .stderr(predicate::str::contains("panicked").not());
    }
}
