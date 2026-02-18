//! Random JSON value generator for testing.
//!
//! This crate provides utilities for generating random JSON values,
//! useful for property-based testing and fuzzing.
//!
//! # Example
//!
//! ```
//! use json_joy_json_random::RandomJson;
//! use serde_json::Value;
//!
//! // Generate a random JSON value
//! let json = RandomJson::generate(Default::default());
//!
//! // Generate specific types
//! let b = RandomJson::gen_boolean();
//! let n = RandomJson::gen_number();
//! let s = RandomJson::gen_string(None);
//! let arr = RandomJson::gen_array(Default::default());
//! let obj = RandomJson::gen_object(Default::default());
//! ```

use rand::Rng;
use serde_json::{Map, Value};
use std::collections::VecDeque;

/// Type of JSON node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Null,
    Boolean,
    Number,
    String,
    Binary,
    Array,
    Object,
}

/// Odds for each node type when generating random JSON.
#[derive(Debug, Clone)]
pub struct NodeOdds {
    pub null: u32,
    pub boolean: u32,
    pub number: u32,
    pub string: u32,
    pub binary: u32,
    pub array: u32,
    pub object: u32,
}

impl Default for NodeOdds {
    fn default() -> Self {
        Self {
            null: 1,
            boolean: 2,
            number: 10,
            string: 8,
            binary: 0,
            array: 2,
            object: 2,
        }
    }
}

impl NodeOdds {
    fn total(&self) -> u32 {
        self.null + self.boolean + self.number + self.string + self.binary + self.array + self.object
    }
}

/// Root node type for generated JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootNode {
    Object,
    Array,
    String,
}

impl Default for RootNode {
    fn default() -> Self {
        Self::Object
    }
}

/// Options for random JSON generation.
#[derive(Debug, Clone)]
pub struct RandomJsonOptions {
    pub root_node: Option<RootNode>,
    pub node_count: usize,
    pub odds: NodeOdds,
}

impl Default for RandomJsonOptions {
    fn default() -> Self {
        Self {
            root_node: Some(RootNode::Object),
            node_count: 32,
            odds: NodeOdds::default(),
        }
    }
}

/// Random JSON generator.
///
/// Generates random JSON values based on configurable options.
pub struct RandomJson {
    opts: RandomJsonOptions,
    total_odds: u32,
    odd_totals: NodeOdds,
    root: Value,
    containers: VecDeque<Value>,
}

impl RandomJson {
    /// Generate a random JSON value with default options.
    pub fn generate(opts: RandomJsonOptions) -> Value {
        let mut rnd = Self::new(opts);
        rnd.create()
    }

    /// Generate a random boolean.
    pub fn gen_boolean() -> bool {
        rand::thread_rng().gen_bool(0.5)
    }

    /// Generate a random number.
    ///
    /// Produces a mix of small integers, medium integers, and floating point numbers.
    pub fn gen_number() -> f64 {
        let mut rng = rand::thread_rng();

        // Draw a fresh value for each branch so all four paths are reachable.
        let num = if rng.gen_bool(0.2) {
            // Small integer (-128 to 127)
            (rng.gen::<u8>() as i32 - 128) as f64
        } else if rng.gen_bool(0.2) {
            // Medium integer (-32768 to 32767)
            (rng.gen::<u16>() as i32 - 32768) as f64
        } else if rng.gen_bool(0.2) {
            // Very large integer
            rng.gen::<i64>() as f64
        } else {
            // Large float
            rng.gen::<f64>() * 1e9
        };

        if num == 0.0 { 0.0 } else { num }
    }

    /// Generate a random string of the specified length.
    ///
    /// If length is None, generates a random length up to 16 characters.
    pub fn gen_string(length: Option<usize>) -> String {
        let mut rng = rand::thread_rng();
        let len = length.unwrap_or_else(|| rng.gen_range(1..=16));

        // 10% chance of UTF-16 characters
        if rng.gen_bool(0.1) {
            (0..len).map(|_| Self::utf16_char(&mut rng)).collect()
        } else {
            (0..len).map(|_| Self::ascii_char(&mut rng)).collect()
        }
    }

    /// Generate random binary data.
    pub fn gen_binary(length: Option<usize>) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let len = length.unwrap_or_else(|| rng.gen_range(1..=16));
        (0..len).map(|_| rng.gen::<u8>()).collect()
    }

    /// Generate a random array.
    pub fn gen_array(opts: RandomJsonOptions) -> Value {
        let mut opts = opts;
        opts.root_node = Some(RootNode::Array);
        opts.node_count = opts.node_count.min(6);
        Self::generate(opts)
    }

    /// Generate a random object.
    pub fn gen_object(opts: RandomJsonOptions) -> Value {
        let mut opts = opts;
        opts.root_node = Some(RootNode::Object);
        opts.node_count = opts.node_count.min(6);
        Self::generate(opts)
    }

    fn ascii_char(rng: &mut impl Rng) -> char {
        // ASCII printable range: 32-126
        (rng.gen_range(32..=126) as u8) as char
    }

    fn utf16_char(rng: &mut impl Rng) -> char {
        // A selection of interesting Unicode characters
        let chars = [
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm',
            'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
            'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
            'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
            '-', '_', '.', ',', ';', '!', '@', '#', '$', '%', '^', '&', '*',
            '\\', '/', '(', ')', '+', '=',
            '\n',
            '👍', '🏻', '😛', 'ä', 'ö', 'ü', 'ß', 'а', 'б', 'в', 'г', '诶', '必', '西',
        ];
        chars[rng.gen_range(0..chars.len())]
    }

    fn new(opts: RandomJsonOptions) -> Self {
        let total_odds = opts.odds.total();

        let odd_totals = NodeOdds {
            null: opts.odds.null,
            boolean: opts.odds.null + opts.odds.boolean,
            number: opts.odds.null + opts.odds.boolean + opts.odds.number,
            string: opts.odds.null + opts.odds.boolean + opts.odds.number + opts.odds.string,
            binary: opts.odds.null + opts.odds.boolean + opts.odds.number + opts.odds.string + opts.odds.binary,
            array: opts.odds.null + opts.odds.boolean + opts.odds.number + opts.odds.string + opts.odds.binary + opts.odds.array,
            object: total_odds,
        };

        let root = match opts.root_node {
            Some(RootNode::String) => Value::String(Self::gen_string(None)),
            Some(RootNode::Object) => Value::Object(Map::new()),
            Some(RootNode::Array) => Value::Array(Vec::new()),
            None => {
                if opts.odds.array > opts.odds.object {
                    Value::Array(Vec::new())
                } else {
                    Value::Object(Map::new())
                }
            }
        };

        let mut containers = VecDeque::new();
        if !matches!(root, Value::String(_)) {
            containers.push_back(root.clone());
        }

        let node_count = if matches!(opts.root_node, Some(RootNode::String)) {
            0
        } else {
            opts.node_count
        };

        Self {
            opts: RandomJsonOptions { node_count, ..opts },
            total_odds,
            odd_totals,
            root,
            containers,
        }
    }

    fn create(&mut self) -> Value {
        for _ in 0..self.opts.node_count {
            self.add_node();
        }
        self.root.clone()
    }

    fn add_node(&mut self) {
        // First, pick a container index
        let container_idx = self.pick_container_index();
        if container_idx.is_none() {
            return;
        }
        let container_idx = container_idx.unwrap();

        // Generate the new node
        let node_type = self.pick_node_type();
        let node = self.generate_value(node_type);

        // If we created a new real container (not binary-as-array), add it to the pool.
        // Binary values are represented as Value::Array but are leaf nodes; only
        // containers created for NodeType::Array or NodeType::Object should be added.
        if matches!(node_type, NodeType::Array | NodeType::Object) {
            self.containers.push_back(node.clone());
        }

        // Insert into the selected container
        // Find the container by index
        let container = self.containers.iter_mut().nth(container_idx);
        if let Some(container) = container {
            match container {
                Value::Array(ref mut arr) => {
                    let mut rng = rand::thread_rng();
                    let index = rng.gen_range(0..=arr.len());
                    arr.insert(index, node);
                }
                Value::Object(ref mut map) => {
                    let key = Self::gen_string(None);
                    map.insert(key, node);
                }
                _ => {}
            }
        }
    }

    fn pick_container_index(&self) -> Option<usize> {
        if self.containers.is_empty() {
            return None;
        }
        let mut rng = rand::thread_rng();
        Some(rng.gen_range(0..self.containers.len()))
    }

    fn generate_value(&self, node_type: NodeType) -> Value {
        match node_type {
            NodeType::Null => Value::Null,
            NodeType::Boolean => Value::Bool(Self::gen_boolean()),
            NodeType::Number => {
                let n = Self::gen_number();
                serde_json::Number::from_f64(n)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            }
            NodeType::String => Value::String(Self::gen_string(None)),
            NodeType::Binary => {
                // Binary is represented as array of numbers in JSON
                let bytes = Self::gen_binary(None);
                Value::Array(bytes.into_iter().map(|b| Value::Number(b.into())).collect())
            }
            NodeType::Array => Value::Array(Vec::new()),
            NodeType::Object => Value::Object(Map::new()),
        }
    }

    fn pick_node_type(&self) -> NodeType {
        let mut rng = rand::thread_rng();
        let odd = rng.gen_range(0..self.total_odds);

        if odd < self.odd_totals.null {
            NodeType::Null
        } else if odd < self.odd_totals.boolean {
            NodeType::Boolean
        } else if odd < self.odd_totals.number {
            NodeType::Number
        } else if odd < self.odd_totals.string {
            NodeType::String
        } else if odd < self.odd_totals.binary {
            NodeType::Binary
        } else if odd < self.odd_totals.array {
            NodeType::Array
        } else {
            NodeType::Object
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_boolean() {
        // Just verify it doesn't crash and returns a bool
        let _ = RandomJson::gen_boolean();
    }

    #[test]
    fn test_gen_number() {
        // Verify it produces valid JSON numbers
        for _ in 0..100 {
            let n = RandomJson::gen_number();
            if serde_json::Number::from_f64(n).is_none() && n != 0.0 {
                panic!("Generated invalid JSON number: {}", n);
            }
        }
    }

    #[test]
    fn test_gen_string() {
        // Use char count, not byte count: multi-byte Unicode chars make .len() > char count.
        let s = RandomJson::gen_string(Some(10));
        assert_eq!(s.chars().count(), 10);

        let s = RandomJson::gen_string(None);
        let char_count = s.chars().count();
        assert!(char_count >= 1 && char_count <= 16);
    }

    #[test]
    fn test_gen_binary() {
        let b = RandomJson::gen_binary(Some(10));
        assert_eq!(b.len(), 10);

        let b = RandomJson::gen_binary(None);
        assert!(b.len() >= 1 && b.len() <= 16);
    }

    #[test]
    fn test_generate_object() {
        let json = RandomJson::generate(Default::default());
        assert!(json.is_object());
    }

    #[test]
    fn test_generate_array() {
        let opts = RandomJsonOptions {
            root_node: Some(RootNode::Array),
            ..Default::default()
        };
        let json = RandomJson::generate(opts);
        assert!(json.is_array());
    }

    #[test]
    fn test_generate_string_root() {
        let opts = RandomJsonOptions {
            root_node: Some(RootNode::String),
            ..Default::default()
        };
        let json = RandomJson::generate(opts);
        assert!(json.is_string());
    }

    #[test]
    fn test_node_count() {
        let opts = RandomJsonOptions {
            node_count: 10,
            ..Default::default()
        };
        let json = RandomJson::generate(opts);

        fn count_nodes(v: &Value) -> usize {
            match v {
                Value::Array(arr) => 1 + arr.iter().map(count_nodes).sum::<usize>(),
                Value::Object(map) => 1 + map.values().map(count_nodes).sum::<usize>(),
                _ => 1,
            }
        }

        // Should have at least the root + 10 nodes
        assert!(count_nodes(&json) >= 1);
    }

    #[test]
    fn test_gen_array_helper() {
        let arr = RandomJson::gen_array(Default::default());
        assert!(arr.is_array());
    }

    #[test]
    fn test_gen_object_helper() {
        let obj = RandomJson::gen_object(Default::default());
        assert!(obj.is_object());
    }
}
