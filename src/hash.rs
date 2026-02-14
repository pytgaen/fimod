use anyhow::{bail, Result};
use digest::Digest;
use monty::MontyObject;

/// Names of external functions exposed to Python molds.
pub const EXTERNAL_FUNCTIONS: &[&str] = &["hs_md5", "hs_sha1", "hs_sha256"];

/// Dispatch an external function call to the appropriate hash handler.
pub fn dispatch(name: &str, args: Vec<MontyObject>) -> Result<MontyObject> {
    match name {
        "hs_md5" => hash_fn::<md5::Md5>(args, "hs_md5"),
        "hs_sha1" => hash_fn::<sha1::Sha1>(args, "hs_sha1"),
        "hs_sha256" => hash_fn::<sha2::Sha256>(args, "hs_sha256"),
        _ => bail!("Unknown hash function: {name}"),
    }
}

/// Generic hash function: 1 string arg → hex string lowercase.
fn hash_fn<D: Digest>(args: Vec<MontyObject>, name: &str) -> Result<MontyObject> {
    if args.len() != 1 {
        bail!("{}() takes 1 argument (string), got {}", name, args.len());
    }
    let input = match &args[0] {
        MontyObject::String(s) => s.as_str(),
        _ => bail!("{name}() expects a string argument"),
    };
    let mut hasher = D::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    Ok(MontyObject::String(hex::encode(result)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(val: &str) -> MontyObject {
        MontyObject::String(val.to_string())
    }

    #[test]
    fn test_hs_md5() {
        let result = dispatch("hs_md5", vec![s("hello")]).unwrap();
        assert_eq!(result, s("5d41402abc4b2a76b9719d911017c592"));
    }

    #[test]
    fn test_hs_sha1() {
        let result = dispatch("hs_sha1", vec![s("hello")]).unwrap();
        assert_eq!(result, s("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"));
    }

    #[test]
    fn test_hs_sha256() {
        let result = dispatch("hs_sha256", vec![s("hello")]).unwrap();
        assert_eq!(
            result,
            s("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
        );
    }

    #[test]
    fn test_wrong_type() {
        let result = dispatch("hs_md5", vec![MontyObject::Int(42)]);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_arg_count() {
        let result = dispatch("hs_md5", vec![s("a"), s("b")]);
        assert!(result.is_err());
    }
}
