use std::sync::OnceLock;

use anyhow::{bail, Result};
use fancy_regex::RegexBuilder;
use monty::{DictPairs, MontyObject};

const DEFAULT_BACKTRACK_LIMIT: usize = 100_000;

static BACKTRACK_LIMIT: OnceLock<usize> = OnceLock::new();

fn backtrack_limit() -> usize {
    *BACKTRACK_LIMIT.get_or_init(|| {
        std::env::var("FIMOD_REGEX_BACKTRACK_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_BACKTRACK_LIMIT)
    })
}

/// Compile a regex with a configurable backtrack limit (ReDoS protection).
/// Override the default (100 000) via `FIMOD_REGEX_BACKTRACK_LIMIT` env var.
fn compile_regex(pattern: &str) -> Result<fancy_regex::Regex> {
    RegexBuilder::new(pattern)
        .backtrack_limit(backtrack_limit())
        .build()
        .map_err(|e| anyhow::anyhow!("Invalid regex pattern: {e}"))
}

/// Names of external functions exposed to Python molds.
/// Order matters: it must match the order passed to MontyRun::new().
pub const EXTERNAL_FUNCTIONS: &[&str] = &[
    "re_search",
    "re_match",
    "re_findall",
    "re_sub",
    "re_split",
    "re_search_fancy",
    "re_match_fancy",
    "re_findall_fancy",
    "re_sub_fancy",
    "re_split_fancy",
];

/// Translate Python `re` replacement syntax to fancy-regex syntax.
///
/// Converts:
/// - `\1`..\`\99` → `$1`..\`$99`  (numbered group references)
/// - `\g<name>` → `${name}`        (named group references)
///
/// All other sequences are passed through unchanged.
pub fn python_to_fancy_replacement(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('1'..='9') => {
                let d1 = chars.next().unwrap();
                let mut num = String::new();
                num.push(d1);
                // Optional second digit (\10..\99)
                if matches!(chars.peek(), Some('0'..='9')) {
                    num.push(chars.next().unwrap());
                }
                // Use ${N} to avoid ambiguity with following identifier chars (e.g. \1_foo → ${1}_foo)
                result.push_str(&format!("${{{num}}}"));
            }
            Some('g') => {
                chars.next(); // consume 'g'
                if chars.peek() == Some(&'<') {
                    chars.next(); // consume '<'
                    let name: String = chars.by_ref().take_while(|&c| c != '>').collect();
                    result.push_str(&format!("${{{name}}}"));
                } else {
                    // Malformed \g (not \g<) — keep as-is
                    result.push('\\');
                    result.push('g');
                }
            }
            _ => {
                // \n, \t, \\, etc. — keep as-is
                result.push('\\');
            }
        }
    }

    result
}

/// Dispatch an external function call to the appropriate regex handler.
///
/// `_fancy` variants use fancy-regex `$1`/`${name}` replacement syntax directly.
/// Non-`_fancy` variants use Python `re` syntax `\1`/`\g<name>` (translated internally).
/// For `re_search`, `re_match`, `re_findall`, `re_split` there is no behavioral difference
/// between normal and fancy variants — both are provided for API consistency.
pub fn dispatch(name: &str, args: Vec<MontyObject>) -> Result<MontyObject> {
    match name {
        "re_search" | "re_search_fancy" => re_search(args),
        "re_match" | "re_match_fancy" => re_match(args),
        "re_findall" | "re_findall_fancy" => re_findall(args),
        "re_sub" => re_sub(args, false),
        "re_sub_fancy" => re_sub(args, true),
        "re_split" | "re_split_fancy" => re_split(args),
        _ => bail!("Unknown external function: {name}"),
    }
}

/// Extract a &str from a MontyObject::String, with a label for error messages.
fn expect_string<'a>(obj: &'a MontyObject, label: &str) -> Result<&'a str> {
    match obj {
        MontyObject::String(s) => Ok(s.as_str()),
        _ => bail!("{label} must be a string, got {obj:?}"),
    }
}

/// Count the number of capture groups (excluding group 0) in a compiled regex.
fn capture_group_count(re: &fancy_regex::Regex) -> usize {
    // capture_names() includes group 0 (always None), so subtract 1
    re.capture_names().count().saturating_sub(1)
}

/// Extract numbered groups (1..N) and named groups from a Captures object.
/// Returns (groups_list, named_dict_or_none).
fn extract_groups(
    re: &fancy_regex::Regex,
    caps: &fancy_regex::Captures,
) -> (MontyObject, MontyObject) {
    // Numbered groups: 1..N (skip group 0 = full match)
    let num_groups = capture_group_count(re);
    let groups: Vec<MontyObject> = (1..=num_groups)
        .map(|i| match caps.get(i) {
            Some(m) => MontyObject::String(m.as_str().to_string()),
            None => MontyObject::None,
        })
        .collect();

    // Named groups
    let named_pairs: Vec<(MontyObject, MontyObject)> = re
        .capture_names()
        .filter_map(|opt_name| {
            opt_name.map(|name| {
                let val = match caps.name(name) {
                    Some(m) => MontyObject::String(m.as_str().to_string()),
                    None => MontyObject::None,
                };
                (MontyObject::String(name.to_string()), val)
            })
        })
        .collect();

    let named = if named_pairs.is_empty() {
        MontyObject::None
    } else {
        MontyObject::Dict(DictPairs::from(named_pairs))
    };

    (MontyObject::List(groups), named)
}

/// Build a match result dict with capture groups:
/// {"match": str, "start": int, "end": int, "groups": [str|None, ...], "named": {str: str} | None}
fn captures_to_dict(re: &fancy_regex::Regex, caps: &fancy_regex::Captures) -> MontyObject {
    let full = caps.get(0).expect("group 0 always exists");
    let (groups, named) = extract_groups(re, caps);

    MontyObject::Dict(DictPairs::from(vec![
        (
            MontyObject::String("match".to_string()),
            MontyObject::String(full.as_str().to_string()),
        ),
        (
            MontyObject::String("start".to_string()),
            MontyObject::Int(full.start() as i64),
        ),
        (
            MontyObject::String("end".to_string()),
            MontyObject::Int(full.end() as i64),
        ),
        (MontyObject::String("groups".to_string()), groups),
        (MontyObject::String("named".to_string()), named),
    ]))
}

/// re_search(pattern, text) → {"match", "start", "end", "groups", "named"} or None
fn re_search(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "re_search() takes 2 arguments (pattern, text), got {}",
            args.len()
        );
    }
    let pattern = expect_string(&args[0], "pattern")?;
    let text = expect_string(&args[1], "text")?;

    let re = compile_regex(pattern)?;

    match re.captures(text) {
        Ok(Some(caps)) => Ok(captures_to_dict(&re, &caps)),
        Ok(None) => Ok(MontyObject::None),
        Err(e) => bail!("Regex execution error: {e}"),
    }
}

/// re_match(pattern, text) → {"match", "start", "end", "groups", "named"} or None
/// Anchored to the start of text (like Python's re.match).
fn re_match(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "re_match() takes 2 arguments (pattern, text), got {}",
            args.len()
        );
    }
    let pattern = expect_string(&args[0], "pattern")?;
    let text = expect_string(&args[1], "text")?;

    // Anchor to start if not already anchored
    let anchored = if pattern.starts_with('^') {
        pattern.to_string()
    } else {
        format!("^(?:{pattern})")
    };

    let re = compile_regex(&anchored)?;

    match re.captures(text) {
        Ok(Some(caps)) => Ok(captures_to_dict(&re, &caps)),
        Ok(None) => Ok(MontyObject::None),
        Err(e) => bail!("Regex execution error: {e}"),
    }
}

/// re_findall(pattern, text) → Python-style results:
/// - No capture groups: [full_match, ...]
/// - 1 capture group: [group1, ...]
/// - N capture groups: [[g1, g2, ...], ...]
fn re_findall(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "re_findall() takes 2 arguments (pattern, text), got {}",
            args.len()
        );
    }
    let pattern = expect_string(&args[0], "pattern")?;
    let text = expect_string(&args[1], "text")?;

    let re = compile_regex(pattern)?;
    let num_groups = capture_group_count(&re);

    let mut results = Vec::new();

    for caps_result in re.captures_iter(text) {
        let caps = caps_result.map_err(|e| anyhow::anyhow!("Regex execution error: {e}"))?;

        match num_groups {
            0 => {
                // No capture groups → return full match strings
                if let Some(m) = caps.get(0) {
                    results.push(MontyObject::String(m.as_str().to_string()));
                }
            }
            1 => {
                // Single group → return group value directly
                let val = match caps.get(1) {
                    Some(m) => MontyObject::String(m.as_str().to_string()),
                    None => MontyObject::None,
                };
                results.push(val);
            }
            n => {
                // Multiple groups → return list of group values
                let group_vals: Vec<MontyObject> = (1..=n)
                    .map(|i| match caps.get(i) {
                        Some(m) => MontyObject::String(m.as_str().to_string()),
                        None => MontyObject::None,
                    })
                    .collect();
                results.push(MontyObject::List(group_vals));
            }
        }
    }

    Ok(MontyObject::List(results))
}

/// re_sub(pattern, replacement, text [, count]) → str
/// re_sub_fancy(pattern, replacement, text [, count]) → str
///
/// `count=0` (default) means replace all.
///
/// `re_sub` uses Python `re` syntax: `\1`, `\g<name>`.
/// `re_sub_fancy` uses fancy-regex syntax: `$1`, `${name}`.
fn re_sub(args: Vec<MontyObject>, fancy: bool) -> Result<MontyObject> {
    if args.len() < 3 || args.len() > 4 {
        bail!(
            "re_sub() takes 3-4 arguments (pattern, replacement, text [, count]), got {}",
            args.len()
        );
    }
    let pattern = expect_string(&args[0], "pattern")?;
    let replacement = expect_string(&args[1], "replacement")?;
    let text = expect_string(&args[2], "text")?;

    let count: usize = if args.len() == 4 {
        match &args[3] {
            MontyObject::Int(n) => *n as usize,
            _ => bail!("re_sub() 4th argument must be count (int)"),
        }
    } else {
        0
    };

    let effective_replacement = if fancy {
        replacement.to_string()
    } else {
        python_to_fancy_replacement(replacement)
    };

    let re = compile_regex(pattern)?;
    let result = re.replacen(text, count, effective_replacement.as_str());
    Ok(MontyObject::String(result.into_owned()))
}

/// re_split(pattern, text) → [str, ...]
/// If the pattern has capture groups, captured text is included in the result (Python behavior).
fn re_split(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "re_split() takes 2 arguments (pattern, text), got {}",
            args.len()
        );
    }
    let pattern = expect_string(&args[0], "pattern")?;
    let text = expect_string(&args[1], "text")?;

    let re = compile_regex(pattern)?;
    let num_groups = capture_group_count(&re);

    let mut parts = Vec::new();
    let mut last_end = 0;

    for caps_result in re.captures_iter(text) {
        let caps = caps_result.map_err(|e| anyhow::anyhow!("Regex execution error: {e}"))?;
        let full = caps.get(0).expect("group 0 always exists");

        // Text before the match
        parts.push(MontyObject::String(
            text[last_end..full.start()].to_string(),
        ));

        // Include captured groups in output (Python re.split behavior)
        for i in 1..=num_groups {
            let val = match caps.get(i) {
                Some(m) => MontyObject::String(m.as_str().to_string()),
                None => MontyObject::None,
            };
            parts.push(val);
        }

        last_end = full.end();
    }

    // Text after the last match
    parts.push(MontyObject::String(text[last_end..].to_string()));

    Ok(MontyObject::List(parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(val: &str) -> MontyObject {
        MontyObject::String(val.to_string())
    }

    fn get_dict_field(dict: &MontyObject, key: &str) -> MontyObject {
        match dict {
            MontyObject::Dict(pairs) => {
                let pairs_vec: Vec<_> = pairs.clone().into_iter().collect();
                for (k, v) in &pairs_vec {
                    if let MontyObject::String(k_str) = k {
                        if k_str == key {
                            return v.clone();
                        }
                    }
                }
                panic!("Key '{key}' not found in dict");
            }
            _ => panic!("Expected dict, got {dict:?}"),
        }
    }

    // ── re_search ──

    #[test]
    fn test_re_search_found() {
        let result = dispatch("re_search", vec![s(r"\d+"), s("abc123def")]).unwrap();
        assert_eq!(get_dict_field(&result, "match"), s("123"));
        assert_eq!(get_dict_field(&result, "start"), MontyObject::Int(3));
        assert_eq!(get_dict_field(&result, "end"), MontyObject::Int(6));
        // No capture groups → empty groups list
        assert_eq!(get_dict_field(&result, "groups"), MontyObject::List(vec![]));
        assert_eq!(get_dict_field(&result, "named"), MontyObject::None);
    }

    #[test]
    fn test_re_search_not_found() {
        let result = dispatch("re_search", vec![s(r"\d+"), s("abcdef")]).unwrap();
        assert_eq!(result, MontyObject::None);
    }

    #[test]
    fn test_re_search_with_groups() {
        let result = dispatch("re_search", vec![s(r"(\w+)@(\w+)"), s("user@host")]).unwrap();
        assert_eq!(get_dict_field(&result, "match"), s("user@host"));
        assert_eq!(
            get_dict_field(&result, "groups"),
            MontyObject::List(vec![s("user"), s("host")])
        );
    }

    #[test]
    fn test_re_search_named_groups() {
        let result = dispatch(
            "re_search",
            vec![s(r"(?P<user>\w+)@(?P<domain>\w+)"), s("admin@server")],
        )
        .unwrap();
        assert_eq!(get_dict_field(&result, "match"), s("admin@server"));
        assert_eq!(
            get_dict_field(&result, "groups"),
            MontyObject::List(vec![s("admin"), s("server")])
        );
        let named = get_dict_field(&result, "named");
        assert_eq!(get_dict_field(&named, "user"), s("admin"));
        assert_eq!(get_dict_field(&named, "domain"), s("server"));
    }

    // ── re_match ──

    #[test]
    fn test_re_match_anchored() {
        let result = dispatch("re_match", vec![s(r"\d+"), s("123abc")]).unwrap();
        assert!(matches!(result, MontyObject::Dict(_)));
        assert_eq!(get_dict_field(&result, "match"), s("123"));

        let result = dispatch("re_match", vec![s(r"\d+"), s("abc123")]).unwrap();
        assert_eq!(result, MontyObject::None);
    }

    #[test]
    fn test_re_match_with_groups() {
        let result = dispatch("re_match", vec![s(r"(#+)\s+(.+)"), s("## Hello World")]).unwrap();
        assert_eq!(get_dict_field(&result, "match"), s("## Hello World"));
        assert_eq!(
            get_dict_field(&result, "groups"),
            MontyObject::List(vec![s("##"), s("Hello World")])
        );
    }

    // ── re_findall ──

    #[test]
    fn test_re_findall_no_groups() {
        let result = dispatch("re_findall", vec![s(r"\d+"), s("a1b22c333")]).unwrap();
        match result {
            MontyObject::List(items) => {
                assert_eq!(items, vec![s("1"), s("22"), s("333")]);
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn test_re_findall_single_group() {
        // Python behavior: with 1 group, returns the group content, not the full match
        let result = dispatch("re_findall", vec![s(r"(\d+)@"), s("1@hello 22@world")]).unwrap();
        match result {
            MontyObject::List(items) => {
                assert_eq!(items, vec![s("1"), s("22")]);
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn test_re_findall_multiple_groups() {
        // Python behavior: with N groups, returns list of lists
        let result = dispatch("re_findall", vec![s(r"(\w+)=(\d+)"), s("a=1 b=2")]).unwrap();
        match result {
            MontyObject::List(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], MontyObject::List(vec![s("a"), s("1")]));
                assert_eq!(items[1], MontyObject::List(vec![s("b"), s("2")]));
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn test_re_findall_lookahead() {
        // fancy-regex feature: lookahead
        let result = dispatch(
            "re_findall",
            vec![s(r"\w+(?=@)"), s("user@host admin@server")],
        )
        .unwrap();
        match result {
            MontyObject::List(items) => {
                assert_eq!(items, vec![s("user"), s("admin")]);
            }
            _ => panic!("Expected list"),
        }
    }

    // ── re_sub ──

    #[test]
    fn test_re_sub() {
        let result = dispatch("re_sub", vec![s(r"\s+"), s(" "), s("a  b   c")]).unwrap();
        assert_eq!(result, s("a b c"));
    }

    #[test]
    fn test_re_sub_with_count() {
        // count=1: replace only the first match
        let result = dispatch(
            "re_sub",
            vec![s(r"\d+"), s("X"), s("a1b2c3"), MontyObject::Int(1)],
        )
        .unwrap();
        assert_eq!(result, s("aXb2c3"));
    }

    #[test]
    fn test_re_sub_with_group_ref_python() {
        // \1 syntax — default python mode
        let result = dispatch(
            "re_sub",
            vec![s(r"(\w+)@(\w+)"), s(r"\2/\1"), s("user@host")],
        )
        .unwrap();
        assert_eq!(result, s("host/user"));
    }

    #[test]
    fn test_re_sub_named_group_python() {
        // \g<name> syntax — default python mode
        let result = dispatch(
            "re_sub",
            vec![
                s(r"(?P<user>\w+)@(?P<domain>\w+)"),
                s(r"\g<domain>/\g<user>"),
                s("alice@example"),
            ],
        )
        .unwrap();
        assert_eq!(result, s("example/alice"));
    }

    #[test]
    fn test_re_sub_fancy() {
        // $1 syntax via re_sub_fancy
        let result = dispatch(
            "re_sub_fancy",
            vec![s(r"(\w+)@(\w+)"), s("$2/$1"), s("user@host")],
        )
        .unwrap();
        assert_eq!(result, s("host/user"));
    }

    #[test]
    fn test_re_sub_fancy_named() {
        // ${name} syntax via re_sub_fancy
        let result = dispatch(
            "re_sub_fancy",
            vec![
                s(r"(?P<user>\w+)@(?P<domain>\w+)"),
                s("${domain}/${user}"),
                s("alice@example"),
            ],
        )
        .unwrap();
        assert_eq!(result, s("example/alice"));
    }

    #[test]
    fn test_re_sub_fancy_with_count() {
        // count argument works with re_sub_fancy too
        let result = dispatch(
            "re_sub_fancy",
            vec![s(r"\d+"), s("X"), s("a1b2c3"), MontyObject::Int(1)],
        )
        .unwrap();
        assert_eq!(result, s("aXb2c3"));
    }

    #[test]
    fn test_re_sub_count_zero_means_all() {
        let result = dispatch(
            "re_sub",
            vec![s(r"\d"), s("X"), s("a1b2c3"), MontyObject::Int(0)],
        )
        .unwrap();
        assert_eq!(result, s("aXbXcX"));
    }

    // ── re_split ──

    #[test]
    fn test_re_split() {
        let result = dispatch("re_split", vec![s(r"[,;]\s*"), s("a, b;c, d")]).unwrap();
        match result {
            MontyObject::List(items) => {
                assert_eq!(items, vec![s("a"), s("b"), s("c"), s("d")]);
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn test_re_split_with_capture_groups() {
        // Python behavior: captured groups are included in the result
        let result = dispatch("re_split", vec![s(r"([,;])\s*"), s("a, b;c")]).unwrap();
        match result {
            MontyObject::List(items) => {
                assert_eq!(items, vec![s("a"), s(","), s("b"), s(";"), s("c")]);
            }
            _ => panic!("Expected list"),
        }
    }

    // ── Error cases ──

    #[test]
    fn test_invalid_regex() {
        let result = dispatch("re_search", vec![s(r"[invalid"), s("text")]);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_arg_count() {
        let result = dispatch("re_search", vec![s("pattern")]);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_function() {
        let result = dispatch("re_unknown", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_re_sub_wrong_arg_count() {
        // Too few
        let result = dispatch("re_sub", vec![s("a"), s("b")]);
        assert!(result.is_err());
        // Too many (5 args — mode string no longer accepted)
        let result = dispatch(
            "re_sub",
            vec![s("a"), s("b"), s("c"), MontyObject::Int(1), s("extra")],
        );
        assert!(result.is_err());
    }

    // ── python_to_fancy_replacement ──

    #[test]
    fn test_python_to_fancy_numbered() {
        assert_eq!(python_to_fancy_replacement(r"\1"), "${1}");
        assert_eq!(python_to_fancy_replacement(r"\9"), "${9}");
        assert_eq!(python_to_fancy_replacement(r"\10"), "${10}");
        assert_eq!(python_to_fancy_replacement(r"\1-\2"), "${1}-${2}");
        // _ is an identifier char → ${N} avoids ambiguity with $N_
        assert_eq!(python_to_fancy_replacement(r"\1_\2"), "${1}_${2}");
    }

    #[test]
    fn test_python_to_fancy_named() {
        assert_eq!(python_to_fancy_replacement(r"\g<name>"), "${name}");
        assert_eq!(
            python_to_fancy_replacement(r"\g<user>/\g<domain>"),
            "${user}/${domain}"
        );
    }

    #[test]
    fn test_python_to_fancy_passthrough() {
        // \0 is not a group ref — pass through
        assert_eq!(python_to_fancy_replacement(r"\0"), r"\0");
        // \n, \t — pass through
        assert_eq!(python_to_fancy_replacement(r"\n"), r"\n");
        // already $ syntax — pass through unchanged
        assert_eq!(python_to_fancy_replacement("$1"), "$1");
        // plain text — unchanged
        assert_eq!(python_to_fancy_replacement("hello"), "hello");
    }
}
