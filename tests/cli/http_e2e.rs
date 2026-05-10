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
