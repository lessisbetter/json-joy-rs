//! ModuleType — a namespace of named type aliases.
//!
//! Upstream reference: json-type/src/type/classes/ModuleType/index.ts

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::schema::{KeySchema, ModuleSchema, ObjSchema, Schema};

/// An alias entry in a module.
#[derive(Debug, Clone)]
pub struct AliasEntry {
    pub id: String,
    /// The type schema for this alias.
    pub schema: Schema,
}

/// Inner state of a module (aliases map).
#[derive(Debug, Default)]
pub struct ModuleTypeInner {
    pub aliases: HashMap<String, AliasEntry>,
}

/// A module/namespace of named type aliases.
///
/// Wraps `ModuleTypeInner` in an `Arc<RwLock<>>` for shared ownership.
#[derive(Debug, Clone, Default)]
pub struct ModuleType {
    pub inner: Arc<RwLock<ModuleTypeInner>>,
}

impl ModuleType {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from an upstream `ModuleSchema`.
    pub fn from_module_schema(module: &ModuleSchema) -> Self {
        let mt = Self::new();
        mt.import(module);
        mt
    }

    /// Register a named alias with the given schema. If already exists, returns the existing.
    pub fn alias(&self, id: impl Into<String>, schema: Schema) -> AliasEntry {
        let id = id.into();
        {
            let inner = self.inner.read().unwrap();
            if let Some(existing) = inner.aliases.get(&id) {
                return existing.clone();
            }
        }
        let entry = AliasEntry {
            id: id.clone(),
            schema,
        };
        let mut inner = self.inner.write().unwrap();
        inner.aliases.insert(id, entry.clone());
        entry
    }

    /// Look up an alias by ID.
    pub fn unalias(&self, id: &str) -> Result<AliasEntry, String> {
        let inner = self.inner.read().unwrap();
        inner
            .aliases
            .get(id)
            .cloned()
            .ok_or_else(|| format!("Alias not found: {}", id))
    }

    /// Check if an alias exists.
    pub fn has_alias(&self, id: &str) -> bool {
        let inner = self.inner.read().unwrap();
        inner.aliases.contains_key(id)
    }

    /// Resolve an alias, following ref chains.
    pub fn resolve(&self, id: &str) -> Result<AliasEntry, String> {
        let entry = self.unalias(id)?;
        match &entry.schema {
            Schema::Ref(r) => self.resolve(&r.ref_.clone()),
            _ => Ok(entry),
        }
    }

    /// Import a module schema, expanding `extends` and registering all aliases.
    pub fn import(&self, module: &ModuleSchema) {
        let mut type_map: HashMap<String, Schema> = HashMap::new();
        for alias in &module.keys {
            type_map.insert(alias.key.clone(), *alias.value.clone());
        }

        // Expand obj extends
        let mut expanded_map: HashMap<String, Schema> = HashMap::new();
        for (key, schema) in &type_map {
            if let Schema::Obj(obj) = schema {
                if obj.extends.is_some() {
                    let expanded = expand_obj_extends(obj, &type_map);
                    expanded_map.insert(key.clone(), Schema::Obj(expanded));
                } else {
                    expanded_map.insert(key.clone(), schema.clone());
                }
            } else {
                expanded_map.insert(key.clone(), schema.clone());
            }
        }

        for (id, schema) in expanded_map {
            self.alias(id, schema);
        }
    }

    /// Import a map of named schemas.
    pub fn import_types(&self, aliases: HashMap<String, Schema>) {
        for (id, schema) in aliases {
            self.alias(id, schema);
        }
    }

    /// Export all aliases as a map of schemas.
    pub fn export_types(&self) -> HashMap<String, Schema> {
        let inner = self.inner.read().unwrap();
        inner
            .aliases
            .iter()
            .map(|(k, v)| (k.clone(), v.schema.clone()))
            .collect()
    }
}

/// Expand the `extends` field of an ObjSchema, merging parent fields.
fn expand_obj_extends(obj: &ObjSchema, type_map: &HashMap<String, Schema>) -> ObjSchema {
    let mut result_keys: Vec<KeySchema> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    let add_key =
        |result_keys: &mut Vec<KeySchema>, seen: &mut HashMap<String, usize>, key: KeySchema| {
            if let Some(&idx) = seen.get(&key.key) {
                result_keys[idx] = key;
            } else {
                seen.insert(key.key.clone(), result_keys.len());
                result_keys.push(key);
            }
        };

    if let Some(extends) = &obj.extends {
        for parent_id in extends {
            if let Some(Schema::Obj(parent)) = type_map.get(parent_id) {
                let parent_expanded = if parent.extends.is_some() {
                    expand_obj_extends(parent, type_map)
                } else {
                    parent.clone()
                };
                for key in parent_expanded.keys {
                    add_key(&mut result_keys, &mut seen, key);
                }
            }
        }
    }
    for key in &obj.keys {
        add_key(&mut result_keys, &mut seen, key.clone());
    }

    ObjSchema {
        base: obj.base.clone(),
        keys: result_keys,
        extends: None,
        decode_unknown_keys: obj.decode_unknown_keys,
        encode_unknown_keys: obj.encode_unknown_keys,
    }
}
