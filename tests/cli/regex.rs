use super::helpers::{setup_input, setup_mold};
use predicates::prelude::*;

#[test]
fn test_re_search_found() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"text": "order-12345-confirmed"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"re_search(r"(\d+)", data["text"])"#])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""match": "12345""#));
}

#[test]
fn test_re_search_not_found() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"text": "no digits here"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"re_search(r"\d+", data["text"])"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("null"));
}

#[test]
fn test_re_match_anchored() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"text": "123abc"}"#);

    // re_match should match at the start
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"re_match(r"\d+", data["text"])"#])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""match": "123""#));

    // re_match should NOT match if pattern is not at start
    let input2 = setup_input(&dir, "data2.json", r#"{"text": "abc123"}"#);
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input2, "-e", r#"re_match(r"\d+", data["text"])"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("null"));
}

#[test]
fn test_re_findall() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"text": "a1 b22 c333"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"re_findall(r"\d+", data["text"])"#])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""1"#))
        .stdout(predicate::str::contains(r#""22"#))
        .stdout(predicate::str::contains(r#""333"#));
}

#[test]
fn test_re_findall_emails() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "data.json",
        r#"{"content": "Contact alice@example.com or bob@test.org for info"}"#,
    );

    let script = r#"
def transform(data, args, env, headers):
    emails = re_findall(r"\w+@\w+\.\w+", data["content"])
    return {"emails": emails}
"#;
    let mold = setup_mold(&dir, "find_emails.py", script);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice@example.com"))
        .stdout(predicate::str::contains("bob@test.org"));
}

#[test]
fn test_re_sub() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"text": "a  b   c    d"}"#);

    let script = r#"
def transform(data, args, env, headers):
    return {"cleaned": re_sub(r"\s+", " ", data["text"])}
"#;
    let mold = setup_mold(&dir, "clean.py", script);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""cleaned": "a b c d""#));
}

#[test]
fn test_re_split() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"text": "one,two; three,four"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"re_split(r"[,;]\s*", data["text"])"#])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""one"#))
        .stdout(predicate::str::contains(r#""two"#))
        .stdout(predicate::str::contains(r#""three"#))
        .stdout(predicate::str::contains(r#""four"#));
}

#[test]
fn test_re_findall_lookahead() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"text": "user@host admin@server"}"#);

    // Lookahead: match word chars before @
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"re_findall(r"\w+(?=@)", data["text"])"#,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""user"#))
        .stdout(predicate::str::contains(r#""admin"#));
}

#[test]
fn test_re_multiple_calls_in_mold() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"text": "  Hello   World  123  "}"#);

    let script = r#"
def transform(data, args, env, headers):
    cleaned = re_sub(r"\s+", " ", data["text"].strip())
    words = re_findall(r"[a-zA-Z]+", cleaned)
    numbers = re_findall(r"\d+", cleaned)
    return {"cleaned": cleaned, "words": words, "numbers": numbers}
"#;
    let mold = setup_mold(&dir, "multi_regex.py", script);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""cleaned": "Hello World 123""#))
        .stdout(predicate::str::contains(r#""Hello"#))
        .stdout(predicate::str::contains(r#""World"#))
        .stdout(predicate::str::contains(r#""123"#));
}

#[test]
fn test_re_with_lines_format() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "access.log",
        "GET /api/users 200\nPOST /api/login 401\nGET /api/health 200\nDELETE /api/users/5 403\n",
    );

    // Filter lines matching error status codes (4xx)
    let script = r#"
def transform(data, args, env, headers):
    return [l for l in data if re_search(r"\s4\d{2}$", l)]
"#;
    let mold = setup_mold(&dir, "filter_errors.py", script);

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "--input-format", "lines", "-m", &mold])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("401"));
    assert!(stdout.contains("403"));
    assert!(!stdout.contains("200"));
}
