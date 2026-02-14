use super::helpers::{setup_input, setup_mold};
use predicates::prelude::*;

#[test]
fn test_readme_filter_reshape() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "data.json",
        r#"[
            {"name": "Alice", "age": 35, "role": "admin", "email": "alice@test.com"},
            {"name": "Bob", "age": 25, "role": "user", "email": "bob@test.com"},
            {"name": "Charlie", "age": 45, "role": "admin", "email": "charlie@test.com"}
        ]"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i", &input,
            "-e", r#"[{"user": u["name"], "email": u["email"]} for u in data if u["age"] > 30 and u["role"] == "admin"]"#,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"user\": \"Alice\""))
        .stdout(predicate::str::contains("\"user\": \"Charlie\""))
        .stdout(predicate::str::contains("bob@test.com").not());
}

#[test]
fn test_readme_conditional_transformation() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "data.json",
        r#"[
            {"first_name": "Alice", "last_name": "Smith", "score": 95},
            {"first_name": "Bob", "last_name": "Jones", "score": 55},
            {"first_name": "Eve", "score": 40}
        ]"#,
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i", &input,
            "-e", r#"[{"name": f"{item['first_name']} {item.get('last_name', 'Unknown')}", "score": item["score"], "status": "excellent" if item["score"] >= 90 else "good" if item["score"] >= 70 else "average" if item["score"] >= 50 else "poor"} for item in data]"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("\"name\": \"Alice Smith\""));
    assert!(stdout.contains("\"status\": \"excellent\""));
    assert!(stdout.contains("\"status\": \"average\""));
    assert!(stdout.contains("\"status\": \"poor\""));
    assert!(stdout.contains("\"name\": \"Eve Unknown\""));
}

#[test]
fn test_readme_aggregation() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "employees.json",
        r#"[
            {"department": "eng", "salary": 100},
            {"department": "eng", "salary": 120},
            {"department": "sales", "salary": 80}
        ]"#,
    );

    let mold = setup_mold(
        &dir,
        "agg.py",
        r#"def transform(data, args, env, headers):
    depts = {}
    for e in data:
        d = e["department"]
        if d not in depts:
            depts[d] = {"dept": d, "count": 0, "total": 0}
        entry = depts[d]
        entry["count"] = entry["count"] + 1
        entry["total"] = entry["total"] + e["salary"]
    result = []
    for entry in depts.values():
        result.append({"dept": entry["dept"], "count": entry["count"], "avg_salary": entry["total"] / entry["count"]})
    return result
"#,
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("\"dept\": \"eng\""));
    assert!(stdout.contains("\"count\": 2"));
    assert!(stdout.contains("\"avg_salary\": 110")); // (100+120)/2
    assert!(stdout.contains("\"dept\": \"sales\""));
}

#[test]
fn test_readme_csv_normalize() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "employees.csv",
        "first_name,last_name,email,dept\n alice , smith ,Alice@Test.COM,eng\n alice , smith ,alice@test.com,eng\nbob,jones,bob@test.com,\n",
    );

    let mold = setup_mold(
        &dir,
        "normalize.py",
        r#"def transform(data, args, env, headers):
    seen = {}
    result = []
    for row in data:
        email = row["email"].strip().lower()
        if email in seen:
            continue
        seen[email] = True
        result.append({
            "name": f"{row['first_name'].strip().title()} {row['last_name'].strip().title()}",
            "email": email,
            "department": (row.get("dept") or "unknown").upper(),
        })
    return result
"#,
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--output-format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("\"name\": \"Alice Smith\""));
    assert!(stdout.contains("\"email\": \"alice@test.com\""));
    assert!(stdout.contains("\"department\": \"ENG\""));
    assert!(stdout.contains("\"name\": \"Bob Jones\""));
    // Deduplicated — only 2 entries
    assert_eq!(stdout.matches("\"email\"").count(), 2);
}

#[test]
fn test_readme_skylos_to_gitlab() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "skylos.json",
        r#"{"unused_functions": [{"name": "foo", "file": "src/lib.rs", "line": 10}], "unused_imports": [{"name": "bar", "file": "src/main.rs", "line": 3}]}"#,
    );

    let mold = setup_mold(
        &dir,
        "skylos.py",
        r#"def transform(data, args, env, headers):
    issues = []
    categories = {
        "unused_functions": "unused-function",
        "unused_imports":   "unused-import",
        "unused_variables": "unused-variable",
    }
    for key, check_name in categories.items():
        for item in data.get(key, []):
            name = item.get("name", "unknown")
            path = item.get("file", "unknown")
            line = item.get("line", 1)
            issues.append({
                "description": f"Unused {key.replace('unused_', '')}: {name}",
                "check_name": check_name,
                "fingerprint": f"{check_name}:{path}:{name}",
                "severity": "info",
                "location": {"path": path, "lines": {"begin": int(line)}}
            })
    return issues
"#,
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("\"description\": \"Unused functions: foo\""));
    assert!(stdout.contains("\"check_name\": \"unused-function\""));
    assert!(stdout.contains("\"description\": \"Unused imports: bar\""));
    assert!(stdout.contains("\"severity\": \"info\""));
    assert!(stdout.contains("\"begin\": 10"));
}

#[test]
fn test_readme_log_counting() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "app.log",
        "ERROR: disk full\nINFO: started\nERROR: timeout\nWARN: slow query\n",
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i", &input,
            "--input-format", "lines",
            "--output-format", "json",
            "-e", "def transform(data, args, env, headers):\n    levels = {}\n    for line in data:\n        for level in [\"ERROR\", \"WARN\", \"INFO\", \"DEBUG\"]:\n            if level in line:\n                levels[level] = levels.get(level, 0) + 1\n    return levels",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("\"ERROR\": 2"));
    assert!(stdout.contains("\"WARN\": 1"));
    assert!(stdout.contains("\"INFO\": 1"));
}

#[test]
fn test_readme_log_filter() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "app.log",
        "ERROR: disk full\nINFO: started\nERROR: timeout\nWARN: slow\n",
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "--input-format",
            "lines",
            "--output-format",
            "lines",
            "-e",
            "[l for l in data if \"ERROR\" in l]",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("ERROR: disk full"));
    assert!(stdout.contains("ERROR: timeout"));
    assert!(!stdout.contains("INFO"));
    assert!(!stdout.contains("WARN"));
}

#[test]
fn test_readme_yaml_to_toml() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "config.yaml", "host: localhost\nport: 8080\n");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "data", "--output-format", "toml"])
        .assert()
        .success()
        .stdout(predicate::str::contains("host = \"localhost\""))
        .stdout(predicate::str::contains("port = 8080"));
}

#[test]
fn test_readme_dict_comprehension_reshape() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "compose.json",
        r#"{"services": {"web": {"image": "nginx", "ports": ["80"]}, "db": {"image": "postgres"}}}"#,
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i", &input,
            "-e", r#"{name: {"image": svc["image"], "ports": svc.get("ports", [])} for name, svc in data["services"].items()}"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("\"web\""));
    assert!(stdout.contains("\"nginx\""));
    assert!(stdout.contains("\"db\""));
    assert!(stdout.contains("\"postgres\""));
}

#[test]
fn test_readme_api_reshape_to_csv() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "orders.json",
        r#"{"orders": [
            {"id": 1, "customer": {"name": "Alice"}, "total": 99, "lines": [1, 2], "shipping": {"address": {"country": "FR"}}},
            {"id": 2, "customer": {"name": "Bob"}, "total": 150, "lines": [1], "shipping": {"address": {"country": "US"}}}
        ]}"#,
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i", &input,
            "--output-format", "csv",
            "-e", r#"[{"id": o["id"], "customer": o["customer"]["name"], "total": o["total"], "items": len(o["lines"]), "country": o["shipping"]["address"]["country"]} for o in data["orders"]]"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("id,customer,total,items,country"));
    assert!(stdout.contains("Alice"));
    assert!(stdout.contains("Bob"));
    assert!(stdout.contains("FR"));
    assert!(stdout.contains("US"));
}
