//! Bound values: what a declared config key holds where the unit runs,
//! read from how the shell registered it through the key's `bind` (the
//! argument after `--store`, the registered command, an env value) or a
//! declared path. See project.md "My harness", "The bring-in contract".

use crate::contract::Declaration;
use crate::installed::{Installed, Registration};
use std::collections::BTreeMap;

/// Every declared config key that binds to a value here.
pub fn values(
    d: &Declaration,
    registration: Option<&Registration>,
    installed: &dyn Installed,
) -> BTreeMap<String, String> {
    d.config
        .iter()
        .filter_map(|(k, key)| {
            let bind = key.bind.as_deref()?;
            Some((k.clone(), value(bind, registration, installed)?))
        })
        .collect()
}

fn value(bind: &str, r: Option<&Registration>, installed: &dyn Installed) -> Option<String> {
    let (how, what) = bind.split_once(':').unwrap_or((bind, ""));
    match how {
        "arg" => {
            let args = &r?.args;
            let at = args.iter().position(|a| a == what)?;
            args.get(at + 1).cloned()
        }
        "command" => {
            let r = r?;
            let all: Vec<&str> = std::iter::once(r.command.as_str())
                .chain(r.args.iter().map(String::as_str))
                .collect();
            Some(serde_json::json!(all).to_string())
        }
        // Only the shell's own `${NAME}` expansion binds, as the value named;
        // a value registered as written is never held.
        "env" => {
            let v = r?.env.get(what)?;
            let name = v.strip_prefix("${")?.strip_suffix('}')?;
            Some(format!("env:{name}"))
        }
        "value" => Some(what.to_string()),
        "path" => expand(what, &|n| installed.var(n)),
        _ => None,
    }
}

/// `$NAME`, `${NAME}` and `${NAME:-default}` (the default expanded too); a
/// variable unset with no default leaves nothing bound.
pub fn expand(template: &str, var: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        if let Some(inner) = rest.strip_prefix('{') {
            let end = closing(inner)?;
            let body = &inner[..end];
            rest = &inner[end + 1..];
            match body.split_once(":-") {
                Some((name, default)) => match var(name) {
                    Some(v) => out.push_str(&v),
                    None => out.push_str(&expand(default, var)?),
                },
                None => out.push_str(&var(body)?),
            }
        } else {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            out.push_str(&var(&rest[..end])?);
            rest = &rest[end..];
        }
    }
    out.push_str(rest);
    Some(out)
}

/// The `}` that closes a `${`, nested ones counted.
fn closing(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' if depth == 0 => return Some(i),
            '}' => depth -= 1,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Key;
    use crate::installed::Unit;

    struct Env(&'static [(&'static str, &'static str)]);
    impl Installed for Env {
        fn units(&self, _: Option<&std::path::Path>) -> anyhow::Result<Vec<Unit>> {
            Ok(vec![])
        }
        fn var(&self, n: &str) -> Option<String> {
            self.0
                .iter()
                .find(|(k, _)| *k == n)
                .map(|(_, v)| v.to_string())
        }
    }

    fn reg() -> Registration {
        Registration {
            command: "/bin/srv".into(),
            args: vec!["--store".into(), "/s".into(), "--flag".into()],
            env: [
                ("TOKEN".to_string(), "${MY_TOKEN}".to_string()),
                ("PLAIN".to_string(), "literal".to_string()),
            ]
            .into(),
        }
    }

    #[test]
    fn each_bind_reads_its_place() {
        let e = Env(&[("HOME", "/h")]);
        let r = reg();
        let v = |b| value(b, Some(&r), &e);
        assert_eq!(v("arg:--store").as_deref(), Some("/s"));
        assert_eq!(v("arg:--flag"), None, "no value after it");
        assert_eq!(v("arg:--gone"), None);
        assert_eq!(
            v("command").as_deref(),
            Some(r#"["/bin/srv","--store","/s","--flag"]"#)
        );
        assert_eq!(v("env:TOKEN").as_deref(), Some("env:MY_TOKEN"));
        assert_eq!(v("env:PLAIN"), None, "a value as written binds nothing");
        assert_eq!(
            v("value:memory/MEMORY.md").as_deref(),
            Some("memory/MEMORY.md")
        );
        assert_eq!(v("path:$HOME/x").as_deref(), Some("/h/x"));
        assert_eq!(value("arg:--store", None, &e), None, "nothing registered");
        assert_eq!(v("other:x"), None);
    }

    #[test]
    fn a_path_expands_defaults_and_leaves_nothing_for_an_unset_variable() {
        let e = |pairs| move |n: &str| Env(pairs).var(n);
        let t = "${MAID_CONFIG:-${XDG_CONFIG_HOME:-$HOME/.config}/maid}/w/learned.md";
        assert_eq!(
            expand(t, &e(&[("HOME", "/h")])).as_deref(),
            Some("/h/.config/maid/w/learned.md")
        );
        assert_eq!(
            expand(t, &e(&[("HOME", "/h"), ("XDG_CONFIG_HOME", "/x")])).as_deref(),
            Some("/x/maid/w/learned.md")
        );
        assert_eq!(
            expand(t, &e(&[("MAID_CONFIG", "/m")])).as_deref(),
            Some("/m/w/learned.md")
        );
        assert_eq!(expand(t, &e(&[])), None);
        assert_eq!(expand("${UNCLOSED", &e(&[("UNCLOSED", "x")])), None);
    }

    #[test]
    fn only_keys_with_a_bind_are_bound() {
        let mut d: Declaration = serde_json::from_value(serde_json::json!(
            {"contract": 1, "scope": "session", "job": "j"}))
        .unwrap();
        d.config.insert(
            "store".into(),
            Key {
                bind: Some("arg:--store".into()),
                ..Key::default()
            },
        );
        d.config.insert("other".into(), Key::default());
        let got = values(&d, Some(&reg()), &Env(&[]));
        assert_eq!(got, [("store".to_string(), "/s".to_string())].into());
    }
}
