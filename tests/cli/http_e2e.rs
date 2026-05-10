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
