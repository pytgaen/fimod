use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{bail, Result};
use minijinja::Environment;
use monty::MontyObject;

use crate::convert::monty_to_json;
use crate::monty_args::expect_string_owned;
use crate::serde_compat::NativeNumbers;

/// Process-wide cache of compiled `minijinja::Environment`s, keyed by
/// `(hash(template_source), auto_escape)` to avoid cloning the source on
/// every lookup. The stored `Arc<str>` is the original source — compared on
/// hit to detect collisions (rare but theoretically possible with the std
/// SipHash). On collision, we recompile (correctness preserved, cache lossy).
///
/// Unbounded by design — same rationale as `regex.rs::REGEX_CACHE`: CLI runs
/// are short-lived; long-running library users with data-derived templates
/// should clear the cache themselves.
type TemplateCache = HashMap<(u64, bool), (Arc<str>, Arc<Environment<'static>>)>;
static TPL_CACHE: OnceLock<Mutex<TemplateCache>> = OnceLock::new();

fn hash_template(s: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Names of external functions exposed to Python molds.
pub const EXTERNAL_FUNCTIONS: &[&str] = &["tpl_render_str", "tpl_render_from_mold"];

/// Dispatch an external function call to the appropriate template handler.
pub fn dispatch(
    name: &str,
    args: Vec<MontyObject>,
    mold_base_dir: Option<&str>,
) -> Result<MontyObject> {
    match name {
        "tpl_render_str" => tpl_render_str(args),
        "tpl_render_from_mold" => tpl_render_from_mold(args, mold_base_dir),
        _ => bail!("Unknown template function: {name}"),
    }
}

/// Extract the common (template_str, ctx, auto_escape) from args.
fn parse_render_args(args: &[MontyObject], fn_name: &str) -> Result<(MontyObject, bool)> {
    if args.is_empty() {
        bail!("{fn_name}() requires at least 2 arguments (template, ctx)");
    }
    // ctx is the second arg (index 1), default to None
    let ctx = if args.len() > 1 {
        args[1].clone()
    } else {
        bail!("{fn_name}() requires at least 2 arguments (template, ctx)");
    };

    // auto_escape is the optional third arg, default False
    let auto_escape = if args.len() > 2 {
        match &args[2] {
            MontyObject::Bool(b) => *b,
            _ => bail!("{fn_name}() auto_escape must be a bool"),
        }
    } else {
        false
    };

    Ok((ctx, auto_escape))
}

/// Render a Jinja2 template string and return the rendered text.
fn render(template_str: &str, ctx: MontyObject, auto_escape: bool) -> Result<MontyObject> {
    let ctx_json = monty_to_json(ctx)?;
    let ctx_value = minijinja::Value::from_serialize(NativeNumbers(&ctx_json));

    let env = get_or_compile_env(template_str, auto_escape)?;
    let tmpl = env.get_template("__inline__").unwrap();
    let rendered = tmpl
        .render(ctx_value)
        .map_err(|e| anyhow::anyhow!("Template render error: {e}"))?;

    Ok(MontyObject::String(rendered))
}

fn get_or_compile_env(template_str: &str, auto_escape: bool) -> Result<Arc<Environment<'static>>> {
    let cache = TPL_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (hash_template(template_str), auto_escape);
    let mut cache = cache.lock().unwrap();
    if let Some((stored, env)) = cache.get(&key) {
        if stored.as_ref() == template_str {
            return Ok(env.clone());
        }
        // Hash collision: fall through to recompile and overwrite. Correctness
        // preserved; the previous entry is evicted (acceptable since collisions
        // are astronomically rare in practice).
    }

    let mut env = Environment::new();
    env.set_trim_blocks(true);
    env.set_lstrip_blocks(true);
    if auto_escape {
        env.set_auto_escape_callback(|_| minijinja::AutoEscape::Html);
    }
    env.add_template_owned("__inline__", template_str.to_string())
        .map_err(|e| anyhow::anyhow!("Template syntax error: {e}"))?;
    let env = Arc::new(env);
    cache.insert(key, (Arc::from(template_str), env.clone()));
    Ok(env)
}

/// `tpl_render_str(template, ctx, auto_escape=False)` — Render a Jinja2 template string.
fn tpl_render_str(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() < 2 || args.len() > 3 {
        bail!(
            "tpl_render_str() takes 2-3 arguments (template, ctx, auto_escape=False), got {}",
            args.len()
        );
    }

    let template_str = expect_string_owned(&args[0], "tpl_render_str() first argument (template)")?;

    let (ctx, auto_escape) = parse_render_args(&args, "tpl_render_str")?;
    render(&template_str, ctx, auto_escape)
}

/// `tpl_render_from_mold(path, ctx, auto_escape=False)` — Load a template file relative
/// to the mold's directory and render it.
fn tpl_render_from_mold(
    args: Vec<MontyObject>,
    mold_base_dir: Option<&str>,
) -> Result<MontyObject> {
    if args.len() < 2 || args.len() > 3 {
        bail!(
            "tpl_render_from_mold() takes 2-3 arguments (path, ctx, auto_escape=False), got {}",
            args.len()
        );
    }

    let base_dir = mold_base_dir.ok_or_else(|| {
        anyhow::anyhow!(
            "tpl_render_from_mold() requires a file-based or registry mold (not inline expressions)"
        )
    })?;

    let rel_path = expect_string_owned(&args[0], "tpl_render_from_mold() first argument (path)")?;

    // Security: resolve and check the path stays under base_dir
    let base = Path::new(base_dir)
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Cannot resolve mold base directory '{base_dir}': {e}"))?;
    let target = base.join(&rel_path);
    let target_canon = target
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Cannot resolve template path '{}': {e}", target.display()))?;
    if !target_canon.starts_with(&base) {
        bail!(
            "tpl_render_from_mold() path traversal denied: '{rel_path}' is outside the mold directory"
        );
    }

    let template_str = std::fs::read_to_string(&target_canon)
        .map_err(|e| anyhow::anyhow!("Cannot read template '{}': {e}", target_canon.display()))?;

    let (ctx, auto_escape) = parse_render_args(&args, "tpl_render_from_mold")?;
    render(&template_str, ctx, auto_escape)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(val: &str) -> MontyObject {
        MontyObject::String(val.to_string())
    }

    fn dict(pairs: Vec<(&str, MontyObject)>) -> MontyObject {
        MontyObject::Dict(monty::DictPairs::from(
            pairs
                .into_iter()
                .map(|(k, v)| (MontyObject::String(k.to_string()), v))
                .collect::<Vec<_>>(),
        ))
    }

    #[test]
    fn test_render_str_basic() {
        let result = dispatch(
            "tpl_render_str",
            vec![s("Hello {{ name }}!"), dict(vec![("name", s("world"))])],
            None,
        )
        .unwrap();
        assert_eq!(result, s("Hello world!"));
    }

    #[test]
    fn test_render_str_loop() {
        let items = MontyObject::List(vec![s("a"), s("b"), s("c")]);
        let result = dispatch(
            "tpl_render_str",
            vec![
                s("{% for x in items %}{{ x }},{% endfor %}"),
                dict(vec![("items", items)]),
            ],
            None,
        )
        .unwrap();
        assert_eq!(result, s("a,b,c,"));
    }

    #[test]
    fn test_render_str_auto_escape() {
        let result = dispatch(
            "tpl_render_str",
            vec![
                s("{{ content }}"),
                dict(vec![("content", s("<b>bold</b>"))]),
                MontyObject::Bool(true),
            ],
            None,
        )
        .unwrap();
        assert_eq!(result, s("&lt;b&gt;bold&lt;&#x2f;b&gt;"));
    }

    #[test]
    fn test_render_str_no_escape_default() {
        let result = dispatch(
            "tpl_render_str",
            vec![
                s("{{ content }}"),
                dict(vec![("content", s("<b>bold</b>"))]),
            ],
            None,
        )
        .unwrap();
        assert_eq!(result, s("<b>bold</b>"));
    }

    #[test]
    fn test_render_from_mold_basic() {
        let dir = tempfile::tempdir().unwrap();
        let tpl_path = dir.path().join("hello.j2");
        std::fs::write(&tpl_path, "Hello {{ name }}!").unwrap();

        let result = dispatch(
            "tpl_render_from_mold",
            vec![s("hello.j2"), dict(vec![("name", s("fimod"))])],
            Some(dir.path().to_str().unwrap()),
        )
        .unwrap();
        assert_eq!(result, s("Hello fimod!"));
    }

    #[test]
    fn test_render_from_mold_no_base_dir() {
        let result = dispatch(
            "tpl_render_from_mold",
            vec![s("hello.j2"), dict(vec![])],
            None,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires a file-based or registry mold"));
    }

    #[test]
    fn test_render_from_mold_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        // Create a file outside the dir
        let outside = dir.path().parent().unwrap().join("secret.txt");
        std::fs::write(&outside, "secret").unwrap();

        let result = dispatch(
            "tpl_render_from_mold",
            vec![s("../secret.txt"), dict(vec![])],
            Some(dir.path().to_str().unwrap()),
        );
        // Clean up
        let _ = std::fs::remove_file(&outside);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path traversal"));
    }

    #[test]
    fn test_wrong_arg_count() {
        let result = dispatch("tpl_render_str", vec![s("template")], None);
        assert!(result.is_err());
    }
}
