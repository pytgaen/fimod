use std::collections::HashMap;

use anyhow::{bail, Result};

/// Result of an HTTP fetch operation.
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    /// Raw response bytes. Always populated; used for binary output pass-through.
    pub body_bytes: Vec<u8>,
    pub content_type: Option<String>,
}

/// Check if a string looks like an HTTP(S) URL.
pub fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Map a Content-Type header value to a fimod format name.
///
/// Returns `None` for unrecognized content types.
pub fn content_type_to_format(ct: &str) -> Option<&'static str> {
    // Extract the MIME type before any parameters (e.g. charset)
    let mime = ct.split(';').next().unwrap_or(ct).trim();
    match mime {
        "application/json" => Some("json"),
        "application/x-ndjson" => Some("ndjson"),
        "text/csv" => Some("csv"),
        "text/tab-separated-values" => Some("tsv"),
        "application/x-yaml" | "text/yaml" | "text/x-yaml" | "application/yaml" => Some("yaml"),
        "application/toml" | "text/toml" => Some("toml"),
        "text/plain" | "text/html" => Some("txt"),
        _ => None,
    }
}

#[cfg(feature = "reqwest")]
fn parse_header(h: &str) -> Result<(&str, &str)> {
    let (name, value) = h
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("Invalid header (expected 'Name: Value'): {h}"))?;
    Ok((name.trim(), value.trim()))
}

#[cfg(feature = "reqwest")]
fn build_client(timeout: u64, no_follow: bool) -> Result<reqwest::blocking::Client> {
    use std::time::Duration;
    let redirect_policy = if no_follow {
        reqwest::redirect::Policy::none()
    } else {
        reqwest::redirect::Policy::default()
    };
    reqwest::blocking::ClientBuilder::new()
        .timeout(Duration::from_secs(timeout))
        .redirect(redirect_policy)
        .user_agent(concat!("fimod/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {e}"))
}

/// Fetch a URL via HTTP GET.
///
/// - `headers`: custom headers as "Name: Value" strings
/// - `timeout`: request timeout in seconds
/// - `no_follow`: if true, don't follow redirects
/// - `debug`: if true, print debug info to stderr
#[cfg(feature = "reqwest")]
pub fn fetch_url(
    url: &str,
    headers: &[String],
    timeout: u64,
    no_follow: bool,
    debug: bool,
) -> Result<HttpResponse> {
    let client = build_client(timeout, no_follow)?;

    let mut request = client.get(url);

    for h in headers {
        let (name, value) = parse_header(h)?;
        request = request.header(name, value);
    }

    if debug {
        eprintln!("[debug] HTTP GET {url}");
        if no_follow {
            eprintln!("[debug] redirects: disabled");
        }
    }

    let resp = request
        .send()
        .map_err(|e| anyhow::anyhow!("HTTP request failed for {url}: {e}"))?;

    let status = resp.status().as_u16();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let resp_headers: HashMap<String, String> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    if debug {
        eprintln!("[debug] HTTP {status} {url}");
        if let Some(ref ct) = content_type {
            eprintln!("[debug] content-type: {ct}");
        }
    }

    let bytes = resp
        .bytes()
        .map_err(|e| anyhow::anyhow!("Failed to read HTTP response body: {e}"))?;
    let body_bytes = bytes.to_vec();

    // For non-redirect statuses, check for errors (but allow 3xx when no_follow is set)
    if status >= 400 {
        bail!("HTTP {status} for {url}");
    }

    // Detect binary content: if the content-type maps to no known text format and
    // doesn't start with "text/", expose body as empty string (raw bytes are in body_bytes).
    let is_binary = content_type
        .as_deref()
        .map(|ct| content_type_to_format(ct).is_none() && !ct.starts_with("text/"))
        .unwrap_or(false);

    let body = if is_binary {
        String::new()
    } else {
        String::from_utf8_lossy(&body_bytes).into_owned()
    };

    Ok(HttpResponse {
        status,
        headers: resp_headers,
        body,
        body_bytes,
        content_type,
    })
}

/// Fetch a URL and return the raw bytes (for binary mode).
///
/// Same parameters as `fetch_url`, but returns `Vec<u8>` without text decoding.
#[cfg(feature = "reqwest")]
pub fn fetch_url_bytes(
    url: &str,
    headers: &[String],
    timeout: u64,
    no_follow: bool,
    debug: bool,
) -> Result<Vec<u8>> {
    let client = build_client(timeout, no_follow)?;

    let mut request = client.get(url);

    for h in headers {
        let (name, value) = parse_header(h)?;
        request = request.header(name, value);
    }

    if debug {
        eprintln!("[debug] HTTP GET {url} (binary)");
        if no_follow {
            eprintln!("[debug] redirects: disabled");
        }
    }

    let resp = request
        .send()
        .map_err(|e| anyhow::anyhow!("HTTP request failed for {url}: {e}"))?;

    let status = resp.status().as_u16();
    if debug {
        eprintln!("[debug] HTTP {status} {url}");
    }

    if status >= 400 {
        bail!("HTTP {status} for {url}");
    }

    let bytes = resp
        .bytes()
        .map_err(|e| anyhow::anyhow!("Failed to read HTTP response body: {e}"))?;

    Ok(bytes.to_vec())
}

#[cfg(not(feature = "reqwest"))]
pub fn fetch_url_bytes(
    url: &str,
    _headers: &[String],
    _timeout: u64,
    _no_follow: bool,
    _debug: bool,
) -> Result<Vec<u8>> {
    bail!(
        "HTTP input is not available (slim build without reqwest): {}",
        url
    )
}

#[cfg(not(feature = "reqwest"))]
pub fn fetch_url(
    url: &str,
    _headers: &[String],
    _timeout: u64,
    _no_follow: bool,
    _debug: bool,
) -> Result<HttpResponse> {
    bail!(
        "HTTP input is not available (slim build without reqwest): {}",
        url
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_url() {
        assert!(is_url("http://example.com"));
        assert!(is_url("https://api.github.com/repos"));
        assert!(!is_url("data.json"));
        assert!(!is_url("/tmp/file.json"));
        assert!(!is_url("./relative.json"));
    }

    #[test]
    fn test_content_type_to_format() {
        assert_eq!(content_type_to_format("application/json"), Some("json"));
        assert_eq!(
            content_type_to_format("application/json; charset=utf-8"),
            Some("json")
        );
        assert_eq!(content_type_to_format("text/csv"), Some("csv"));
        assert_eq!(content_type_to_format("text/yaml"), Some("yaml"));
        assert_eq!(content_type_to_format("application/toml"), Some("toml"));
        assert_eq!(content_type_to_format("text/plain"), Some("txt"));
        assert_eq!(content_type_to_format("text/html"), Some("txt"));
        assert_eq!(
            content_type_to_format("application/x-ndjson"),
            Some("ndjson")
        );
        assert_eq!(content_type_to_format("application/octet-stream"), None);
        assert_eq!(content_type_to_format("image/png"), None);
    }
}
