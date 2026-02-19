//! Random value generator for type nodes.
//!
//! Upstream reference: json-type/src/random/Random.ts

use json_joy_json_random::{RandomJson, RandomJsonOptions};
use rand::Rng;
use serde_json::Value;

use crate::type_def::classes::{
    ArrType, BinType, BoolType, MapType, NumType, ObjType, OrType, StrType,
};
use crate::type_def::TypeNode;

/// Generates random JSON values that conform to a given TypeNode schema.
pub struct Random;

impl Random {
    pub fn new() -> Self {
        Self
    }

    /// Generate a random value matching the given TypeNode.
    pub fn gen(&self, type_: &TypeNode) -> Value {
        match type_ {
            TypeNode::Any(_) => self.gen_any(),
            TypeNode::Bool(t) => self.gen_bool(t),
            TypeNode::Num(t) => self.gen_num(t),
            TypeNode::Str(t) => self.gen_str(t),
            TypeNode::Bin(t) => self.gen_bin(t),
            TypeNode::Con(t) => t.value.clone(),
            TypeNode::Arr(t) => self.gen_arr(t),
            TypeNode::Obj(t) => self.gen_obj(t),
            TypeNode::Map(t) => self.gen_map(t),
            TypeNode::Or(t) => self.gen_or(t),
            TypeNode::Ref(t) => {
                // Try to resolve ref via system
                if let Some(system) = &t.base.system {
                    if let Ok(alias) = system.resolve(&t.ref_) {
                        return self.gen(
                            &crate::type_def::builder::TypeBuilder::new().import(&alias.schema),
                        );
                    }
                }
                Value::Null
            }
            TypeNode::Fn(_) | TypeNode::FnRx(_) => Value::Null,
            TypeNode::Key(t) => self.gen(t.val.as_ref()),
            TypeNode::Alias(t) => self.gen(t.type_.as_ref()),
        }
    }

    fn gen_any(&self) -> Value {
        RandomJson::generate(RandomJsonOptions {
            root_node: None,
            node_count: 5,
            ..Default::default()
        })
    }

    fn gen_bool(&self, _t: &BoolType) -> Value {
        Value::Bool(RandomJson::gen_boolean())
    }

    fn gen_num(&self, t: &NumType) -> Value {
        let mut rng = rand::thread_rng();
        let schema = &t.schema;

        let is_int = schema.format.map(|f| f.is_integer()).unwrap_or(false);
        let is_uint = schema.format.map(|f| f.is_unsigned()).unwrap_or(false);

        let lo = schema
            .gt
            .map(|v| v + 1.0)
            .or(schema.gte)
            .unwrap_or(f64::MIN);
        let hi = schema
            .lt
            .map(|v| v - 1.0)
            .or(schema.lte)
            .unwrap_or(f64::MAX);

        let (lo, hi) = if lo > hi { (hi, lo) } else { (lo, hi) };
        let range = if hi - lo > 1_000_000.0 {
            1_000_000.0
        } else {
            hi - lo
        };

        let v = lo + rng.gen::<f64>() * range;
        let v = if is_int { v.round() } else { v };
        let v = if is_uint && v < 0.0 { -v } else { v };

        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .unwrap_or(Value::Number(0.into()))
    }

    fn gen_str(&self, t: &StrType) -> Value {
        let schema = &t.schema;
        let min = schema.min.unwrap_or(0) as usize;
        let max = schema.max.map(|v| v as usize).unwrap_or(16).max(min);
        let len = rand::thread_rng().gen_range(min..=max);
        let is_ascii = schema
            .format
            .map(|f| matches!(f, crate::schema::StrFormat::Ascii))
            .unwrap_or(false)
            || schema.ascii.unwrap_or(false);
        let s = if is_ascii {
            (0..len)
                .map(|_| rand::thread_rng().gen_range(32u8..=126) as char)
                .collect::<String>()
        } else {
            RandomJson::gen_string(Some(len))
        };
        Value::String(s)
    }

    fn gen_bin(&self, _t: &BinType) -> Value {
        let bytes = RandomJson::gen_binary(None);
        Value::Array(bytes.into_iter().map(|b| Value::Number(b.into())).collect())
    }

    fn gen_arr(&self, t: &ArrType) -> Value {
        let mut result = Vec::new();
        for h in &t.head {
            result.push(self.gen(h));
        }
        if let Some(el_type) = &t.type_ {
            let schema = &t.schema;
            let min = schema.min.unwrap_or(0) as usize;
            let max = schema.max.map(|v| v as usize).unwrap_or(5).max(min);
            let count = rand::thread_rng().gen_range(min..=max);
            for _ in 0..count {
                result.push(self.gen(el_type));
            }
        }
        for tail in &t.tail {
            result.push(self.gen(tail));
        }
        Value::Array(result)
    }

    fn gen_obj(&self, t: &ObjType) -> Value {
        let mut map = serde_json::Map::new();
        let schema = &t.schema;
        if schema.decode_unknown_keys == Some(true) {
            if let Value::Object(extra) = RandomJson::gen_object(Default::default()) {
                for (k, v) in extra {
                    map.insert(k, v);
                }
            }
        }
        for field in &t.keys {
            if field.optional && rand::thread_rng().gen_bool(0.5) {
                continue;
            }
            map.insert(field.key.clone(), self.gen(field.val.as_ref()));
        }
        Value::Object(map)
    }

    fn gen_map(&self, t: &MapType) -> Value {
        let count = rand::thread_rng().gen_range(0..=5usize);
        let mut map = serde_json::Map::new();
        for _ in 0..count {
            let key = RandomJson::gen_string(None);
            let val = self.gen(t.value.as_ref());
            map.insert(key, val);
        }
        Value::Object(map)
    }

    fn gen_or(&self, t: &OrType) -> Value {
        if t.types.is_empty() {
            return Value::Null;
        }
        let idx = rand::thread_rng().gen_range(0..t.types.len());
        self.gen(&t.types[idx])
    }
}

impl Default for Random {
    fn default() -> Self {
        Self::new()
    }
}
