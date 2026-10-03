use super::helpers::setup_mold;
use predicates::prelude::*;

const LEGACY_CALLS: &[&str] = &[
    "re_search('a', 'a')",
    "re_match('a', 'a')",
    "re_findall('a', 'a')",
    "re_sub('a', 'b', 'a')",
    "re_split(',', 'a,b')",
    "re_search_fancy('a', 'a')",
    "re_match_fancy('a', 'a')",
    "re_findall_fancy('a', 'a')",
    "re_sub_fancy('a', 'b', 'a')",
    "re_split_fancy(',', 'a,b')",
    "it_unique([1, 1])",
    "it_unique_by([{'id': 1}], 'id')",
    "it_flatten([1, [2]])",
];

#[test]
fn test_legacy_builtins_disabled_without_explicit_opt_in() {
    for value in [None, Some("0"), Some("true")] {
        for expression in LEGACY_CALLS {
            let name = expression.split('(').next().unwrap();
            let mut cmd = assert_cmd::cargo_bin_cmd!("fimod");
            cmd.env_remove("FIMOD_LEGACY_BUILTINS");
            if let Some(value) = value {
                cmd.env("FIMOD_LEGACY_BUILTINS", value);
            }
            cmd.args(["s", "--no-input", "-e", expression])
                .assert()
                .failure()
                .stdout("")
                .stderr(predicate::str::contains(format!(
                    "{name}() is deprecated and disabled by default"
                )))
                .stderr(predicate::str::contains("FIMOD_LEGACY_BUILTINS=1"));
        }
    }
}

#[test]
fn test_legacy_builtins_opt_in_preserves_results_without_warnings() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "legacy.py",
        r#"
def transform(data, env, **_):
    return {
        "calls": [re_search("a", "éa")["start"],
                  re_split_fancy("(,)", "a,b"),
                  re_sub_fancy("(a)", "$1!", "a"),
                  it_unique([1, True, 1.0, 1, [2], [2]]),
                  it_unique_by([{"id": [1], "n": 1}, {"id": [1], "n": 2}], "id"),
                  it_flatten([1, (2, [3]), {"x": [4]}])],
        "env": env,
    }
"#,
    );
    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .env("FIMOD_LEGACY_BUILTINS", "1")
        .args(["s", "--no-input", "-m", &mold])
        .assert()
        .success()
        .stderr("")
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "calls": [2, ["a", ",", "b"], "a!", [1, true, 1.0, [2]],
                      [{"id": [1], "n": 1}], [1, 2, 3, {"x": [4]}]],
            "env": {},
        })
    );
}

#[test]
fn test_legacy_alias_cannot_bypass_opt_in() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "alias.py",
        "def transform(data, **_):\n    helper = re_search\n    return helper('a', 'a')\n",
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .env_remove("FIMOD_LEGACY_BUILTINS")
        .args(["s", "--no-input", "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains("use `import re`"));
}

#[test]
fn test_unused_legacy_call_does_not_block_native_python() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "native.py",
        r#"
import re

def transform(data, **_):
    if False:
        it_unique([1, 1])
    return re.findall(r"(?<=foo)bar", "foobar")
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .env_remove("FIMOD_LEGACY_BUILTINS")
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
        .stdout("[\"bar\"]\n")
        .stderr("");
}
