use anyhow::{bail, Result};
use monty_types::MontyObject;

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
    let template = match args[0].as_ref().as_str() {
        Some(s) => s,
        _ => bail!("env_subst() expects a string as first argument"),
    };
    if !matches!(
        monty_types::unstable::root_node(&args[1]),
        monty_types::unstable::MontyNode::Dict(_)
    ) {
        bail!("env_subst() expects a dict as second argument");
    }
    let dict = args[1].as_ref();

    let result = substitute(template, dict);
    Ok(MontyObject::string(result))
}

/// Replace `${VAR}` patterns in template using values from dict.
/// Unmatched variables are left as-is.
fn substitute(template: &str, dict: monty_types::ObjectRef<'_>) -> String {
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

/// Look up a key and return its text representation if found.
fn lookup(dict: monty_types::ObjectRef<'_>, key: &str) -> Option<String> {
    let v = crate::monty_args::field(dict, key)?;
    Some(match monty_types::unstable::node(v) {
        monty_types::unstable::MontyNode::None => String::new(),
        monty_types::unstable::MontyNode::Bool(b) => b.to_string(),
        _ => v.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(val: &str) -> MontyObject {
        MontyObject::string(val.to_string())
    }

    fn make_dict(pairs: Vec<(&str, &str)>) -> MontyObject {
        MontyObject::dict(
            pairs
                .into_iter()
                .map(|(k, v)| (s(k), s(v)))
                .collect::<Vec<_>>(),
        )
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
        let args = vec![MontyObject::int(1), make_dict(vec![])];
        assert!(dispatch("env_subst", args).is_err());
    }

    #[test]
    fn test_wrong_second_arg_type() {
        let args = vec![s("a"), s("b")];
        assert!(dispatch("env_subst", args).is_err());
    }
}
