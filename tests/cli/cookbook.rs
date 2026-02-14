/// Tests covering the cookbook examples from docs/cookbook.md and docs/guides/mold-scripting.md.
///
/// These tests are deliberately targeted: low-level helpers are already covered in their own
/// files (regex.rs, dotpath.rs, iter_helpers.rs, hash.rs, etc.). This file only tests
/// composite examples or patterns not covered elsewhere.
use super::helpers::{setup_input, setup_mold};

// ── Flat CSV → Nested JSON ────────────────────────────────────────────────
//
// docs/cookbook.md § "Data Structuring / Flat CSV to Nested JSON"

#[test]
fn test_cookbook_csv_to_nested_json() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "permissions.csv",
        "id,role,permission\n101,admin,read\n101,admin,write\n102,user,read\n",
    );
    let mold = setup_mold(
        &dir,
        "nest.py",
        r#"def transform(data, args, env, headers):
    result = {}
    for row in data:
        user_id = row["id"]
        if user_id not in result:
            result[user_id] = {"role": row["role"], "permissions": []}
        result[user_id]["permissions"].append(row["permission"])
    return result
"#,
    );

    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--output-format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(stdout).unwrap();

    assert!(stdout.contains("\"101\""));
    assert!(stdout.contains("\"role\": \"admin\""));
    assert!(stdout.contains("\"read\""));
    assert!(stdout.contains("\"write\""));
    assert!(stdout.contains("\"102\""));
    assert!(stdout.contains("\"role\": \"user\""));
    // Only 2 top-level entries
    assert_eq!(stdout.matches("\"role\"").count(), 2);
}

// ── Masking Sensitive Data ────────────────────────────────────────────────
//
// docs/cookbook.md § "Data Cleaning / Masking Sensitive Data"

#[test]
fn test_cookbook_mask_email() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "users.json",
        r#"[{"email": "alice@example.com"}, {"email": "bob@test.org"}]"#,
    );
    let mold = setup_mold(
        &dir,
        "mask.py",
        r#"def transform(data, args, env, headers):
    for user in data:
        if "email" in user:
            parts = user["email"].split("@")
            user["email"] = f"{parts[0][0]}***@{parts[1]}"
    return data
"#,
    );

    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(stdout).unwrap();

    assert!(stdout.contains("a***@example.com"));
    assert!(stdout.contains("b***@test.org"));
    // Original addresses must not appear
    assert!(!stdout.contains("alice@example.com"));
    assert!(!stdout.contains("bob@test.org"));
}

// ── Regex: extract URLs ───────────────────────────────────────────────────
//
// docs/cookbook.md § "Regex Recipes / Extract URLs"

#[test]
fn test_cookbook_regex_extract_urls() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "page.json",
        r#"{"text": "Visit https://example.com and https://test.org for more"}"#,
    );
    let mold = setup_mold(
        &dir,
        "urls.py",
        r#"def transform(data, args, env, headers):
    urls = re_findall(r"https?://[^\s]+", data["text"])
    return {"urls": urls, "count": len(urls)}
"#,
    );

    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(stdout).unwrap();

    assert!(stdout.contains("https://example.com"));
    assert!(stdout.contains("https://test.org"));
    assert!(stdout.contains("\"count\": 2"));
}

// ── Regex: parse KEY=VALUE strings ───────────────────────────────────────
//
// docs/cookbook.md § "Regex Recipes / Parse Structured Strings"
// Bug fixed in the doc: was `data.strip()`, corrected to `data["text"].strip()`.

#[test]
fn test_cookbook_regex_parse_kv() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "cfg.json",
        r#"{"text": "HOST=localhost\nPORT=5432\nDEBUG=true"}"#,
    );
    let mold = setup_mold(
        &dir,
        "parse_kv.py",
        r#"def transform(data, args, env, headers):
    result = {}
    for line in data["text"].strip().split("\n"):
        m = re_search(r"^(\w+)=(.+)$", line)
        if m:
            key_match = re_search(r"^(\w+)", line)
            val_match = re_search(r"=(.+)$", line)
            if key_match and val_match:
                result[key_match["match"]] = val_match["match"][1:]
    return result
"#,
    );

    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(stdout).unwrap();

    assert!(stdout.contains("\"HOST\""));
    assert!(stdout.contains("\"localhost\""));
    assert!(stdout.contains("\"PORT\""));
    assert!(stdout.contains("\"5432\""));
    assert!(stdout.contains("\"DEBUG\""));
    assert!(stdout.contains("\"true\""));
}

// ── --arg: generic reusable filter ───────────────────────────────────────
//
// docs/cookbook.md § "Parameterized Scripts / Reusable Filter"

#[test]
fn test_cookbook_args_reusable_filter() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "users.json",
        r#"[
            {"role": "admin", "name": "Alice"},
            {"role": "user",  "name": "Bob"},
            {"role": "admin", "name": "Charlie"}
        ]"#,
    );
    let mold = setup_mold(
        &dir,
        "filter_by_field.py",
        r#"def transform(data, args, env, headers):
    field = args["field"]
    value = args["value"]
    return [row for row in data if row.get(field) == value]
"#,
    );

    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "--arg",
            "field=role",
            "--arg",
            "value=admin",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(stdout).unwrap();

    assert!(stdout.contains("\"Alice\""));
    assert!(stdout.contains("\"Charlie\""));
    assert!(!stdout.contains("\"Bob\""));
}

// ── --no-input: generate fixture from args ───────────────────────────────
//
// docs/cookbook.md § "Data Generation / Generate a Fixture from Arguments"

#[test]
fn test_cookbook_no_input_gen_users() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "gen_users.py",
        r#"def transform(data, args, env, headers):
    n = int(args["count"])
    prefix = args.get("prefix", "user")
    return [{"id": i, "name": prefix + str(i), "active": True} for i in range(1, n + 1)]
"#,
    );

    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--no-input",
            "-m",
            &mold,
            "--arg",
            "count=3",
            "--arg",
            "prefix=test",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(stdout).unwrap();

    assert!(stdout.contains("\"name\": \"test1\""));
    assert!(stdout.contains("\"name\": \"test2\""));
    assert!(stdout.contains("\"name\": \"test3\""));
    assert!(stdout.contains("\"active\": true"));
    assert_eq!(stdout.matches("\"id\"").count(), 3);
}

// ── CSV headers global: numeric column aggregation ────────────────────────
//
// docs/guides/mold-scripting.md § "CSV headers global"

#[test]
fn test_cookbook_headers_numeric_cols_sum() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "sales.csv",
        "name,q1_amount,q2_amount,category\nAlice,100,200,eng\nBob,50,75,sales\n",
    );
    let mold = setup_mold(
        &dir,
        "sum_amounts.py",
        r#"def transform(data, args, env, headers):
    numeric_cols = [h for h in headers if h.endswith("_amount")]
    for row in data:
        total = 0
        for c in numeric_cols:
            total = total + float(row[c])
        row["total"] = total
    return data
"#,
    );

    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--output-format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(stdout).unwrap();

    assert!(stdout.contains("\"total\": 300")); // Alice: 100 + 200
    assert!(stdout.contains("\"total\": 125")); // Bob: 50 + 75
}
