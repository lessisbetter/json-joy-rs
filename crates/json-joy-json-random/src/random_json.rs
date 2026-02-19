use rand::Rng;
use serde_json::{Map, Value};

use crate::string::{random_string, Token};

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
        self.null
            + self.boolean
            + self.number
            + self.string
            + self.binary
            + self.array
            + self.object
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
    /// Optional schema for generating strings.
    pub strings: Option<Token>,
}

impl Default for RandomJsonOptions {
    fn default() -> Self {
        Self {
            root_node: Some(RootNode::Object),
            node_count: 32,
            odds: NodeOdds::default(),
            strings: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathSeg {
    Key(String),
    Index(usize),
}

/// Random JSON generator.
pub struct RandomJson {
    opts: RandomJsonOptions,
    total_odds: u32,
    odd_totals: NodeOdds,
    root: Value,
    containers: Vec<Vec<PathSeg>>,
}

impl RandomJson {
    /// Generate a random JSON value.
    pub fn generate(opts: RandomJsonOptions) -> Value {
        Self::new(opts).create()
    }

    /// Generate a random boolean.
    pub fn gen_boolean() -> bool {
        rand::thread_rng().gen_bool(0.5)
    }

    /// Generate a random number.
    pub fn gen_number() -> f64 {
        let mut rng = rand::thread_rng();
        let num = if rng.gen_bool(0.8) {
            rng.gen::<f64>() * 1e9
        } else if rng.gen_bool(0.2) {
            (rng.gen::<u8>() as i32 - 128) as f64
        } else if rng.gen_bool(0.2) {
            (rng.gen::<u16>() as i32 - 32768) as f64
        } else {
            (rng.gen::<i64>() as f64).round()
        };
        if num == 0.0 {
            0.0
        } else {
            num
        }
    }

    /// Generate a random string.
    pub fn gen_string(length: Option<usize>) -> String {
        let mut rng = rand::thread_rng();
        let length = length.unwrap_or_else(|| rng.gen_range(1..=16));
        let mut str_ = String::new();
        if rng.gen_bool(0.1) {
            for _ in 0..length {
                str_.push(Self::utf16_char(&mut rng));
            }
        } else {
            for _ in 0..length {
                str_.push(Self::ascii_char(&mut rng));
            }
        }
        if str_.chars().count() != length {
            return std::iter::repeat_with(|| Self::ascii_char(&mut rng))
                .take(length)
                .collect();
        }
        str_
    }

    /// Generate random binary data.
    pub fn gen_binary(length: Option<usize>) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let length = length.unwrap_or_else(|| rng.gen_range(1..=16));
        (0..length).map(|_| rng.gen::<u8>()).collect()
    }

    /// Generate random array root.
    pub fn gen_array(mut opts: RandomJsonOptions) -> Value {
        opts.root_node = Some(RootNode::Array);
        opts.node_count = 6;
        Self::generate(opts)
    }

    /// Generate random object root.
    pub fn gen_object(mut opts: RandomJsonOptions) -> Value {
        opts.root_node = Some(RootNode::Object);
        opts.node_count = 6;
        Self::generate(opts)
    }

    fn ascii_char(rng: &mut impl Rng) -> char {
        (rng.gen_range(32..=126) as u8) as char
    }

    fn utf16_char(rng: &mut impl Rng) -> char {
        let chars = [
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q',
            'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H',
            'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y',
            'Z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-', '_', '.', ',', ';', '!',
            '@', '#', '$', '%', '^', '&', '*', '\\', '/', '(', ')', '+', '=', '\n', '👍', '🏻',
            '😛', 'ä', 'ö', 'ü', 'ß', 'а', 'б', 'в', 'г', '诶', '必', '西',
        ];
        chars[rng.gen_range(0..chars.len())]
    }

    fn new(mut opts: RandomJsonOptions) -> Self {
        let mut odd_totals = NodeOdds::default();
        odd_totals.null = opts.odds.null;
        odd_totals.boolean = odd_totals.null + opts.odds.boolean;
        odd_totals.number = odd_totals.boolean + opts.odds.number;
        odd_totals.string = odd_totals.number + opts.odds.string;
        odd_totals.binary = odd_totals.string + opts.odds.binary;
        // Intentionally mirrors upstream threshold progression.
        odd_totals.array = odd_totals.string + opts.odds.array;
        odd_totals.object = odd_totals.array + opts.odds.object;
        let total_odds = opts.odds.total();

        let mut containers = Vec::new();
        let root = match opts.root_node {
            Some(RootNode::String) => {
                opts.node_count = 0;
                if let Some(schema) = &opts.strings {
                    Value::String(random_string(schema))
                } else {
                    Value::String(Self::gen_string(None))
                }
            }
            Some(RootNode::Object) => {
                containers.push(Vec::new());
                Value::Object(Map::new())
            }
            Some(RootNode::Array) => {
                containers.push(Vec::new());
                Value::Array(Vec::new())
            }
            None => {
                let is_object = Self::pick_container_type(&opts.odds) == RootNode::Object;
                containers.push(Vec::new());
                if is_object {
                    Value::Object(Map::new())
                } else {
                    Value::Array(Vec::new())
                }
            }
        };

        Self {
            opts,
            total_odds,
            odd_totals,
            root,
            containers,
        }
    }

    fn pick_container_type(odds: &NodeOdds) -> RootNode {
        let sum = odds.array + odds.object;
        if sum == 0 {
            return RootNode::Object;
        }
        if rand::thread_rng().gen::<f64>() < (odds.array as f64 / sum as f64) {
            RootNode::Array
        } else {
            RootNode::Object
        }
    }

    fn create(mut self) -> Value {
        for _ in 0..self.opts.node_count {
            self.add_node();
        }
        self.root
    }

    fn add_node(&mut self) {
        if self.containers.is_empty() {
            return;
        }
        let mut rng = rand::thread_rng();
        let container_idx = rng.gen_range(0..self.containers.len());
        let container_path = self.containers[container_idx].clone();
        let node_type = self.pick_node_type();
        let is_container = matches!(node_type, NodeType::Array | NodeType::Object);
        let node = self.generate_value(node_type);

        let container = match get_mut_by_path(&mut self.root, &container_path) {
            Some(container) => container,
            None => return,
        };

        match container {
            Value::Array(arr) => {
                let index = rng.gen_range(0..=arr.len());
                arr.insert(index, node);
                self.adjust_indices_after_array_insert(&container_path, index);
                if is_container {
                    let mut new_path = container_path;
                    new_path.push(PathSeg::Index(index));
                    self.containers.push(new_path);
                }
            }
            Value::Object(map) => {
                let key = Self::gen_string(None);
                map.insert(key.clone(), node);
                if is_container {
                    let mut new_path = container_path;
                    new_path.push(PathSeg::Key(key));
                    self.containers.push(new_path);
                }
            }
            _ => {}
        }
    }

    fn adjust_indices_after_array_insert(&mut self, base_path: &[PathSeg], inserted_index: usize) {
        for path in &mut self.containers {
            if path.len() <= base_path.len() {
                continue;
            }
            if !path.starts_with(base_path) {
                continue;
            }
            if let Some(PathSeg::Index(idx)) = path.get_mut(base_path.len()) {
                if *idx >= inserted_index {
                    *idx += 1;
                }
            }
        }
    }

    fn generate_value(&self, node_type: NodeType) -> Value {
        match node_type {
            NodeType::Null => Value::Null,
            NodeType::Boolean => Value::Bool(Self::gen_boolean()),
            NodeType::Number => serde_json::Number::from_f64(Self::gen_number())
                .map(Value::Number)
                .unwrap_or(Value::Null),
            NodeType::String => {
                if let Some(schema) = &self.opts.strings {
                    Value::String(random_string(schema))
                } else {
                    Value::String(Self::gen_string(None))
                }
            }
            NodeType::Binary => {
                let bytes = Self::gen_binary(None);
                Value::Array(bytes.into_iter().map(|b| Value::Number(b.into())).collect())
            }
            NodeType::Array => Value::Array(Vec::new()),
            NodeType::Object => Value::Object(Map::new()),
        }
    }

    fn pick_node_type(&self) -> NodeType {
        if self.total_odds == 0 {
            return NodeType::Null;
        }
        let odd = rand::thread_rng().gen::<f64>() * self.total_odds as f64;
        if odd <= self.odd_totals.null as f64 {
            NodeType::Null
        } else if odd <= self.odd_totals.boolean as f64 {
            NodeType::Boolean
        } else if odd <= self.odd_totals.number as f64 {
            NodeType::Number
        } else if odd <= self.odd_totals.string as f64 {
            NodeType::String
        } else if odd <= self.odd_totals.binary as f64 {
            NodeType::Binary
        } else if odd <= self.odd_totals.array as f64 {
            NodeType::Array
        } else {
            NodeType::Object
        }
    }
}

fn get_mut_by_path<'a>(value: &'a mut Value, path: &[PathSeg]) -> Option<&'a mut Value> {
    if path.is_empty() {
        return Some(value);
    }
    match (&path[0], value) {
        (PathSeg::Index(i), Value::Array(arr)) => arr
            .get_mut(*i)
            .and_then(|next| get_mut_by_path(next, &path[1..])),
        (PathSeg::Key(key), Value::Object(map)) => map
            .get_mut(key)
            .and_then(|next| get_mut_by_path(next, &path[1..])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_non_empty_object_when_node_count_positive() {
        let value = RandomJson::generate(RandomJsonOptions {
            root_node: Some(RootNode::Object),
            node_count: 10,
            ..Default::default()
        });
        assert!(value.is_object());
        assert!(value.as_object().is_some_and(|obj| !obj.is_empty()));
    }

    #[test]
    fn generates_string_from_schema() {
        let token = Token::list(vec![
            Token::repeat(2, 2, Token::literal("xx")),
            Token::pick(vec![Token::literal("y")]),
        ]);
        let str_value = RandomJson::generate(RandomJsonOptions {
            root_node: Some(RootNode::String),
            strings: Some(token),
            ..Default::default()
        });
        assert_eq!(str_value, Value::String("xxxxy".to_string()));
    }
}
