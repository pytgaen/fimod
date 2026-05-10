use httpmock::MockServer;
use predicates::prelude::*;
use std::time::Duration;

#[test]
fn test_http_timeout_surfaces_clear_error() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/slow");
        then.status(200)
            .delay(Duration::from_secs(5))
            .body(r#"{"ok": true}"#);
    });

    let url = format!("{}/slow", server.base_url());

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &url, "-e", "data", "--timeout", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("HTTP request failed"));
}

#[test]
fn test_http_401_surfaces_status_code() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/secured");
        then.status(401).body("Unauthorized");
    });

    let url = format!("{}/secured", server.base_url());

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &url, "-e", "data"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("HTTP 401"));
}

#[test]
fn test_http_404_surfaces_status_code() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/missing");
        then.status(404).body("Not Found");
    });

    let url = format!("{}/missing", server.base_url());

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &url, "-e", "data"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("HTTP 404"));
}

#[test]
fn test_http_500_surfaces_status_code() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/boom");
        then.status(500).body("Internal Server Error");
    });

    let url = format!("{}/boom", server.base_url());

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &url, "-e", "data"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("HTTP 500"));
}

#[test]
fn test_http_follows_redirect_by_default() {
    let server = MockServer::start();
    let target_url = format!("{}/final", server.base_url());

    let target = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/final");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"final":true}"#);
    });
    let _redir = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/redirect");
        then.status(302).header("location", &target_url);
    });

    let url = format!("{}/redirect", server.base_url());
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &url, "-e", "data"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"final\""));
    target.assert_hits(1);
}

#[test]
fn test_http_content_type_json_is_parsed() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/data");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"key":"value"}"#);
    });

    let url = format!("{}/data", server.base_url());
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &url, "-e", "data"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"key\""))
        .stdout(predicate::str::contains("\"value\""));
}

#[test]
fn test_http_content_type_yaml_is_parsed() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/data");
        then.status(200)
            .header("content-type", "application/yaml")
            .body("key: value\n");
    });

    let url = format!("{}/data", server.base_url());
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &url, "-e", "data", "--output-format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"key\""))
        .stdout(predicate::str::contains("\"value\""));
}

#[test]
fn test_http_content_type_csv_is_parsed() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/data");
        then.status(200)
            .header("content-type", "text/csv")
            .body("name,age\nalice,30\nbob,42\n");
    });

    let url = format!("{}/data", server.base_url());
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &url, "-e", "data"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice"))
        .stdout(predicate::str::contains("bob"));
}

#[test]
fn test_http_content_type_with_charset_suffix_is_parsed() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/data");
        then.status(200)
            .header("content-type", "application/json; charset=utf-8")
            .body(r#"{"key":"value"}"#);
    });

    let url = format!("{}/data", server.base_url());
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &url, "-e", "data"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"key\""))
        .stdout(predicate::str::contains("\"value\""));
}

#[test]
fn test_http_no_follow_returns_3xx_body_without_following() {
    let server = MockServer::start();
    let target_url = format!("{}/final", server.base_url());

    let target = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/final");
        then.status(200).body(r#"{"final":true}"#);
    });
    let _redir = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/redirect");
        then.status(302)
            .header("location", &target_url)
            .header("content-type", "application/json")
            .body(r#"{"hint":"go elsewhere"}"#);
    });

    let url = format!("{}/redirect", server.base_url());
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &url, "-e", "data", "--no-follow"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"hint\""));
    target.assert_hits(0);
}
