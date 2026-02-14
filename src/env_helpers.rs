use anyhow::{bail, Result};
use monty::{DictPairs, MontyObject};

/// Names of external functions exposed to Python molds.
pub const EXTERNAL_FUNCTIONS: &[&str] = &["env_subst"];

/// Dispatch an external function call to the appropriate env handler.
pub fn dispatch(name: &str, args: Vec<MontyObject>) -> Result<MontyObject> {
    match name {
        "env_subst" => dispatch_env_subst(args),
        _ => bail!("Unknown env_helpers function: {name}"),
    }
}

/// env_subst(template, dict) — substitute ${VAR} placeholders using the provided dict.
/// Unknown variables are left as-is (standard envsubst behavior).
fn dispatch_env_subst(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "env_subst() takes 2 arguments (template, dict), got {}",
            args.len()
        );
    }
    let template = match &args[0] {
        MontyObject::String(s) => s.as_str(),
        _ => bail!("env_subst() expects a string as first argument"),
    };
    let dict = match &args[1] {
        MontyObject::Dict(d) => d,
        _ => bail!("env_subst() expects a dict as second argument"),
    };

    let result = substitute(template, dict);
    Ok(MontyObject::String(result))
}

/// Replace `${VAR}` patterns in template using values from dict.
/// Unmatched variables are left as-is.
fn substitute(template: &str, dict: &DictPairs) -> String {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut var_name = String::new();
            let mut found_close = false;
            for ch in chars.by_ref() {
                if ch == '}' {
                    found_close = true;
                    break;
                }
                var_name.push(ch);
            }
            if found_close {
                match lookup(dict, &var_name) {
                    Some(val) => result.push_str(&val),
                    None => {
                        result.push_str("${");
                        result.push_str(&var_name);
                        result.push('}');
                    }
                }
            } else {
                // Unclosed ${..., output literally
                result.push_str("${");
                result.push_str(&var_name);
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Look up a key in DictPairs, return the string value if found.
fn lookup(dict: &DictPairs, key: &str) -> Option<String> {
    for (k, v) in dict {
        if let MontyObject::String(k_str) = k {
            if k_str == key {
                return match v {
                    MontyObject::String(s) => Some(s.clone()),
                    MontyObject::Int(i) => Some(i.to_string()),
                    MontyObject::Float(f) => Some(f.to_string()),
                    MontyObject::Bool(b) => Some(b.to_string()),
                    MontyObject::None => Some(String::new()),
                    _ => Some(format!("{v:?}")),
                };
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(val: &str) -> MontyObject {
        MontyObject::String(val.to_string())
    }

    fn make_dict(pairs: Vec<(&str, &str)>) -> MontyObject {
        MontyObject::Dict(DictPairs::from(
            pairs
                .into_iter()
                .map(|(k, v)| (s(k), s(v)))
                .collect::<Vec<_>>(),
        ))
    }

    #[test]
    fn test_basic_substitution() {
        let args = vec![
            s("https://${HOST}/api"),
            make_dict(vec![("HOST", "example.com")]),
        ];
        let result = dispatch("env_subst", args).unwrap();
        assert_eq!(result, s("https://example.com/api"));
    }

    #[test]
    fn test_multiple_vars() {
        let args = vec![
            s("${PROTO}://${HOST}:${PORT}"),
            make_dict(vec![
                ("PROTO", "https"),
                ("HOST", "example.com"),
                ("PORT", "443"),
            ]),
        ];
        let result = dispatch("env_subst", args).unwrap();
        assert_eq!(result, s("https://example.com:443"));
    }

    #[test]
    fn test_unknown_var_left_as_is() {
        let args = vec![
            s("${HOST}/${UNKNOWN}"),
            make_dict(vec![("HOST", "example.com")]),
        ];
        let result = dispatch("env_subst", args).unwrap();
        assert_eq!(result, s("example.com/${UNKNOWN}"));
    }

    #[test]
    fn test_no_vars() {
        let args = vec![s("plain text"), make_dict(vec![])];
        let result = dispatch("env_subst", args).unwrap();
        assert_eq!(result, s("plain text"));
    }

    #[test]
    fn test_dollar_without_brace() {
        let args = vec![s("$HOST is ok"), make_dict(vec![("HOST", "x")])];
        let result = dispatch("env_subst", args).unwrap();
        assert_eq!(result, s("$HOST is ok"));
    }

    #[test]
    fn test_wrong_arg_count() {
        assert!(dispatch("env_subst", vec![s("a")]).is_err());
    }

    #[test]
    fn test_wrong_first_arg_type() {
        let args = vec![MontyObject::Int(1), make_dict(vec![])];
        assert!(dispatch("env_subst", args).is_err());
    }

    #[test]
    fn test_wrong_second_arg_type() {
        let args = vec![s("a"), s("b")];
        assert!(dispatch("env_subst", args).is_err());
    }
}
