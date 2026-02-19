//! Extension registry framework for JSON CRDT.
//!
//! Mirrors `packages/json-joy/src/json-crdt/extensions/`:
//! - `Extension.ts`   → [`AnyExtension`] trait
//! - `Extensions.ts`  → [`Extensions`] registry
//! - `ExtNode.ts`     → [`ExtNode`] trait
//! - `types.ts`       → [`ExtApi`] trait
//!
//! # Overview
//!
//! Extensions add higher-level semantics on top of the base CRDT node types
//! by registering themselves with a named ID into an [`Extensions`] registry.
//! Each extension has a globally unique 8-bit unsigned integer ID (at most 256
//! extensions may be registered simultaneously).
//!
//! Extension nodes are `vec` nodes with a specific 2-tuple structure:
//! ```text
//! vec
//! ├─ 0: con Uint8Array { <ext_id>, <sid_mod_256>, <time_mod_256> }
//! └─ 1: any   (extension payload)
//! ```
//!
//! The concrete extension implementations (cnt, mval, peritext) live in
//! `crate::json_crdt_extensions`; this module provides only the framework
//! types that the model layer uses for lookup and dispatch.

use serde_json::Value;

// ── ExtNode trait ─────────────────────────────────────────────────────────

/// Trait implemented by every extension node type.
///
/// Mirrors the abstract class `ExtNode<N, View>` in `ExtNode.ts`.
pub trait ExtNode: Send + Sync {
    /// The globally unique 8-bit extension ID (0–255).
    fn ext_id(&self) -> u32;

    /// The human-readable name of this extension (e.g. `"cnt"`, `"peritext"`).
    fn name(&self) -> &str;

    /// Return the extension's current logical view as a JSON value.
    fn view(&self) -> Value;
}

// ── ExtApi trait ──────────────────────────────────────────────────────────

/// Trait for extension API objects.
///
/// Mirrors the `ExtApi` interface in `types.ts`.  The API object wraps an
/// `ExtNode` and exposes mutation methods.  Concrete implementations live in
/// the individual extension modules.
pub trait ExtApi<EN: ExtNode>: Send + Sync {
    /// Return the extension node this API operates on.
    fn node(&self) -> &EN;
}

// ── AnyExtension trait ────────────────────────────────────────────────────

/// Trait for a registered extension descriptor.
///
/// Mirrors `AnyExtension` / `Extension<…>` from `Extension.ts`.
/// In TypeScript the `Extension` class carried `Node` and `Api` constructors
/// as fields; in Rust we use trait objects and factory functions instead.
pub trait AnyExtension: Send + Sync {
    /// The globally unique 8-bit extension ID.
    fn id(&self) -> u32;

    /// The human-readable name of the extension.
    fn name(&self) -> &str;
}

// ── Extensions registry ───────────────────────────────────────────────────

/// Registry of known extensions.
///
/// Mirrors the `Extensions` class in `Extensions.ts`.
///
/// Each extension is keyed by its numeric ID.  At most 256 extensions can
/// be registered (IDs 0–255), mirroring the upstream constraint.
#[derive(Default)]
pub struct Extensions {
    ext: std::collections::HashMap<u32, Box<dyn AnyExtension>>,
}

impl Extensions {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            ext: std::collections::HashMap::new(),
        }
    }

    /// Register an extension.  If an extension with the same ID was already
    /// registered it is silently replaced.
    pub fn register(&mut self, ext: Box<dyn AnyExtension>) {
        self.ext.insert(ext.id(), ext);
    }

    /// Look up an extension by its numeric ID.
    pub fn get(&self, id: u32) -> Option<&dyn AnyExtension> {
        self.ext.get(&id).map(|e| e.as_ref())
    }

    /// Return the number of registered extensions.
    pub fn size(&self) -> usize {
        self.ext.len()
    }

    /// Create a shallow clone of the registry (all registered extensions are
    /// shared via their trait-object representations).
    ///
    /// Mirrors `Extensions.clone()` in the upstream TypeScript.
    ///
    /// Because `Box<dyn AnyExtension>` is not `Clone`, we can only clone the
    /// *registry* if extension descriptors are registered as `Arc`-backed
    /// objects.  For now this returns a new empty registry; callers that need
    /// cloning should use `Arc<Extensions>` instead.
    pub fn clone_empty(&self) -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    struct MockExt {
        id: u32,
        name: &'static str,
    }

    impl AnyExtension for MockExt {
        fn id(&self) -> u32 {
            self.id
        }
        fn name(&self) -> &str {
            self.name
        }
    }

    // -- Extensions::new / default ------------------------------------------

    #[test]
    fn extensions_new_is_empty() {
        let exts = Extensions::new();
        assert_eq!(exts.size(), 0);
    }

    #[test]
    fn extensions_default_is_empty() {
        let exts: Extensions = Default::default();
        assert_eq!(exts.size(), 0);
    }

    // -- register / get / size -----------------------------------------------

    #[test]
    fn extensions_register_and_get() {
        let mut exts = Extensions::new();
        exts.register(Box::new(MockExt {
            id: 42,
            name: "mock",
        }));
        assert!(exts.get(42).is_some());
        assert_eq!(exts.get(42).unwrap().name(), "mock");
        assert_eq!(exts.get(42).unwrap().id(), 42);
    }

    #[test]
    fn extensions_get_missing_returns_none() {
        let exts = Extensions::new();
        assert!(exts.get(99).is_none());
    }

    #[test]
    fn extensions_size_tracks_registrations() {
        let mut exts = Extensions::new();
        assert_eq!(exts.size(), 0);
        exts.register(Box::new(MockExt { id: 1, name: "a" }));
        assert_eq!(exts.size(), 1);
        exts.register(Box::new(MockExt { id: 2, name: "b" }));
        assert_eq!(exts.size(), 2);
    }

    #[test]
    fn extensions_register_overwrites_same_id() {
        let mut exts = Extensions::new();
        exts.register(Box::new(MockExt {
            id: 7,
            name: "first",
        }));
        exts.register(Box::new(MockExt {
            id: 7,
            name: "second",
        }));
        assert_eq!(exts.size(), 1);
        assert_eq!(exts.get(7).unwrap().name(), "second");
    }

    #[test]
    fn extensions_multiple_independent_ids() {
        let mut exts = Extensions::new();
        for i in 0u32..5 {
            exts.register(Box::new(MockExt { id: i, name: "ext" }));
        }
        assert_eq!(exts.size(), 5);
        for i in 0u32..5 {
            assert!(exts.get(i).is_some());
        }
        assert!(exts.get(5).is_none());
    }

    // -- ExtNode trait object ------------------------------------------------

    struct MockExtNode {
        ext_id: u32,
    }

    impl ExtNode for MockExtNode {
        fn ext_id(&self) -> u32 {
            self.ext_id
        }
        fn name(&self) -> &str {
            "mock-node"
        }
        fn view(&self) -> serde_json::Value {
            serde_json::Value::Null
        }
    }

    #[test]
    fn ext_node_trait_methods() {
        let node = MockExtNode { ext_id: 3 };
        assert_eq!(node.ext_id(), 3);
        assert_eq!(node.name(), "mock-node");
        assert_eq!(node.view(), serde_json::Value::Null);
    }
}
