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
fn test_monty_15_language_and_stdlib_additions() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = super::helpers::setup_mold(
        &dir,
        "monty15.py",
        r#"
import bisect
import csv
import fnmatch
import hashlib
import os.path
import re
import tomllib
from dataclasses import dataclass, field
from decimal import Decimal

@dataclass
class Order:
    items: list = field(default_factory=list)
    total: int = 0

    def __post_init__(self):
        self.total = sum(self.items)

def kind(value):
    match value:
        case {"type": "point", "x": x, "y": y}:
            return f"point {x},{y}"
        case [first, *rest]:
            return f"list of {1 + len(rest)}"
        case _:
            return "other"

def transform(data, **_):
    return {
        "match": [kind(v) for v in data],
        "sha256": hashlib.sha256(b"fimod").hexdigest()[:12],
        "toml": tomllib.loads('name = "x"\nn = 2'),
        "csv": list(csv.reader(["a,b", "1,2"])),
        "bisect": bisect.bisect_left([1, 3, 5], 4),
        "fnmatch": fnmatch.fnmatch("mold.py", "*.py"),
        "join": os.path.join("molds", "clean.py"),
        "decimal": str(Decimal("19.90") + Decimal("0.10")),
        "split": re.split(r"(\d)", "a1b2c"),
        "post_init": Order([1, 2, 3]).total,
    }
"#,
    );
    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .args(["s", "-m", &mold, "--output-format", "json-compact"])
        .write_stdin(r#"[{"type":"point","x":1,"y":2},[1,2,3],7]"#)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let result: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        result,
        serde_json::json!({
            "match": ["point 1,2", "list of 3", "other"],
            "sha256": "04ef922ce5ac",
            "toml": {"name": "x", "n": 2},
            "csv": [["a", "b"], ["1", "2"]],
            "bisect": 2,
            "fnmatch": true,
            "join": "molds/clean.py",
            "decimal": "20.00",
            "split": ["a", "1", "b", "2", "c"],
            "post_init": 6,
        })
    );
}

#[test]
fn test_monty_decimal_result_is_not_serialized() {
    // Decimal output support is deferred (notes/ROADMAP.md); molds must convert explicitly.
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = super::helpers::setup_mold(
        &dir,
        "decimal.py",
        "from decimal import Decimal\ndef transform(data, **_):\n    return Decimal('1.10')\n",
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["s", "--no-input", "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Decimal(\"1.10\")"));
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
