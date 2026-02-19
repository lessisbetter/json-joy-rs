//! Utility functions for JSON Patch operations.
//!
//! Mirrors `packages/json-joy/src/json-patch/util.ts`.
//!
//! The upstream `util.ts` exports Slate-editor helpers and a regex-matcher
//! factory.  In Rust we expose the path-prefix matcher that is most generally
//! useful for JSON Patch consumers.

use super::types::Op;
use json_joy_json_pointer::Path;

// ── Path matcher ───────────────────────────────────────────────────────────

/// Creates a closure that returns `true` if an `Op`'s path starts with the
/// given `prefix` path.
///
/// An op matches when its path is equal to the prefix **or** longer and has
/// the prefix as an ancestor (i.e. the prefix is a proper prefix of the path).
///
/// This mirrors the path-filtering pattern used in the upstream TypeScript
/// when consumers want to narrow a patch to a specific subtree.
///
/// # Example
///
/// ```
/// use json_joy::json_patch::{Op, util::matcher};
///
/// let prefix: Vec<String> = vec!["foo".to_string()];
/// let is_under_foo = matcher(&prefix);
///
/// let add_under_foo = Op::Add {
///     path: vec!["foo".to_string(), "bar".to_string()],
///     value: serde_json::json!(1),
/// };
/// assert!(is_under_foo(&add_under_foo));
///
/// let add_elsewhere = Op::Add {
///     path: vec!["baz".to_string()],
///     value: serde_json::json!(2),
/// };
/// assert!(!is_under_foo(&add_elsewhere));
/// ```
pub fn matcher(prefix: &Path) -> impl Fn(&Op) -> bool + '_ {
    move |op: &Op| {
        let path = op.path();
        path_starts_with(path, prefix)
    }
}

/// Returns `true` if `path` starts with `prefix` (i.e. prefix is a prefix
/// of path — path == prefix or path has more components and all prefix
/// components match).
pub fn path_starts_with(path: &[String], prefix: &[String]) -> bool {
    if path.len() < prefix.len() {
        return false;
    }
    path[..prefix.len()] == *prefix
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_patch::types::Op;
    use serde_json::json;

    fn s(s: &str) -> String {
        s.to_string()
    }
    fn path(steps: &[&str]) -> Vec<String> {
        steps.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn path_starts_with_same_path() {
        assert!(path_starts_with(
            &path(&["foo", "bar"]),
            &path(&["foo", "bar"])
        ));
    }

    #[test]
    fn path_starts_with_longer_path() {
        assert!(path_starts_with(
            &path(&["foo", "bar", "baz"]),
            &path(&["foo", "bar"])
        ));
    }

    #[test]
    fn path_starts_with_shorter_path_returns_false() {
        assert!(!path_starts_with(&path(&["foo"]), &path(&["foo", "bar"])));
    }

    #[test]
    fn path_starts_with_empty_prefix_always_matches() {
        assert!(path_starts_with(&path(&["foo", "bar"]), &path(&[])));
        assert!(path_starts_with(&path(&[]), &path(&[])));
    }

    #[test]
    fn path_starts_with_different_prefix_returns_false() {
        assert!(!path_starts_with(&path(&["baz"]), &path(&["foo"])));
    }

    #[test]
    fn matcher_matches_op_under_prefix() {
        let prefix = path(&["foo"]);
        let is_match = matcher(&prefix);

        let op = Op::Add {
            path: path(&["foo", "bar"]),
            value: json!(1),
        };
        assert!(is_match(&op));
    }

    #[test]
    fn matcher_matches_op_at_exact_prefix() {
        let prefix = path(&["foo"]);
        let is_match = matcher(&prefix);

        let op = Op::Add {
            path: path(&["foo"]),
            value: json!(1),
        };
        assert!(is_match(&op));
    }

    #[test]
    fn matcher_rejects_op_outside_prefix() {
        let prefix = path(&["foo"]);
        let is_match = matcher(&prefix);

        let op = Op::Add {
            path: path(&["baz"]),
            value: json!(2),
        };
        assert!(!is_match(&op));
    }

    #[test]
    fn matcher_empty_prefix_matches_all() {
        let prefix: Vec<String> = vec![];
        let is_match = matcher(&prefix);

        let op1 = Op::Add {
            path: path(&["foo"]),
            value: json!(1),
        };
        let op2 = Op::Remove {
            path: path(&["a", "b"]),
            old_value: None,
        };
        assert!(is_match(&op1));
        assert!(is_match(&op2));
    }

    #[test]
    fn matcher_works_with_remove_op() {
        let prefix = path(&["a"]);
        let is_match = matcher(&prefix);

        let op = Op::Remove {
            path: path(&["a", "1"]),
            old_value: None,
        };
        assert!(is_match(&op));

        let op2 = Op::Remove {
            path: path(&["b"]),
            old_value: None,
        };
        assert!(!is_match(&op2));
    }
}
