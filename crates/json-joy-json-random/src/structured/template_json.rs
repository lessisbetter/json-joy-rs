use rand::Rng;
use serde_json::{Map, Number, Value};

use crate::number::{int, int64};
use crate::string::random_string;
use crate::util::clone_json;

use super::templates;
use super::types::{ObjectTemplateField, Template};

#[derive(Clone, Copy, Debug, Default)]
pub struct TemplateJsonOpts {
    pub max_nodes: Option<usize>,
}

pub struct TemplateJson {
    template: Template,
    nodes: usize,
    max_nodes: usize,
}

impl TemplateJson {
    pub fn gen(template: Option<Template>, opts: Option<TemplateJsonOpts>) -> Value {
        let template = template.unwrap_or_else(templates::nil);
        let opts = opts.unwrap_or_default();
        let mut generator = Self::new(template, opts);
        generator.generate_current()
    }

    pub fn new(template: Template, opts: TemplateJsonOpts) -> Self {
        Self {
            template,
            nodes: 0,
            max_nodes: opts.max_nodes.unwrap_or(100),
        }
    }

    pub fn generate_current(&mut self) -> Value {
        let template = self.template.clone();
        self.generate(template)
    }

    fn generate(&mut self, mut tpl: Template) -> Value {
        self.nodes += 1;
        while let Template::Recursive(make) = tpl {
            tpl = make();
        }
        match tpl {
            Template::Arr {
                min,
                max,
                item,
                head,
                tail,
            } => self.generate_array(min, max, item, head, tail),
            Template::Obj(fields) => self.generate_object(fields),
            Template::Map {
                key,
                value,
                min,
                max,
            } => self.generate_map(key, value, min, max),
            Template::Str(token) => self.generate_string(token),
            Template::Num(min, max) => self.generate_number(min, max),
            Template::Int(min, max) => self.generate_integer(min, max),
            Template::Int64(min, max) => self.generate_int64(min, max),
            Template::Float(min, max) => self.generate_float(min, max),
            Template::Bool(value) => self.generate_boolean(value),
            Template::Bin {
                min,
                max,
                omin,
                omax,
            } => self.generate_bin(min, max, omin, omax),
            Template::Nil => Value::Null,
            Template::Lit(value) => self.generate_literal(value),
            Template::Or(options) => self.generate_or(options),
            Template::Recursive(_) => unreachable!(),
        }
    }

    fn minmax(&self, min: usize, mut max: usize) -> usize {
        if self.nodes > self.max_nodes {
            return min;
        }
        if self.nodes + max > self.max_nodes {
            max = self.max_nodes.saturating_sub(self.nodes);
        }
        if max < min {
            max = min;
        }
        int(min as i64, max as i64) as usize
    }

    fn generate_array(
        &mut self,
        min: Option<usize>,
        max: Option<usize>,
        item: Option<Box<Template>>,
        head: Vec<Template>,
        tail: Vec<Template>,
    ) -> Value {
        let min = min.unwrap_or(0);
        let max = max.unwrap_or(5);
        let length = self.minmax(min, max);

        let mut result = Vec::new();
        for tpl in head {
            result.push(self.generate(tpl));
        }

        let item_template = item.map(|it| *it).unwrap_or_else(templates::nil);
        for _ in 0..length {
            result.push(self.generate(item_template.clone()));
        }

        for tpl in tail {
            result.push(self.generate(tpl));
        }

        Value::Array(result)
    }

    fn generate_object(&mut self, fields: Vec<ObjectTemplateField>) -> Value {
        let mut result = Map::new();
        for field in fields {
            let optionality = field.optionality.unwrap_or(0.0).clamp(0.0, 1.0);
            if optionality > 0.0 {
                if self.nodes > self.max_nodes {
                    continue;
                }
                if rand::thread_rng().gen::<f64>() < optionality {
                    continue;
                }
            }

            let key = random_string(&field.key.unwrap_or_else(templates::tokens_object_key));
            let value_template = field.value.unwrap_or_else(templates::nil);
            result.insert(key, self.generate(value_template));
        }
        Value::Object(result)
    }

    fn generate_map(
        &mut self,
        key: Option<crate::string::Token>,
        value: Option<Box<Template>>,
        min: Option<usize>,
        max: Option<usize>,
    ) -> Value {
        let min = min.unwrap_or(0);
        let max = max.unwrap_or(5);
        let length = self.minmax(min, max);
        let key_token = key.unwrap_or_else(templates::tokens_object_key);
        let value_template = value.map(|v| *v).unwrap_or_else(templates::nil);

        let mut result = Map::new();
        for _ in 0..length {
            let key = random_string(&key_token);
            let val = self.generate(value_template.clone());
            result.insert(key, val);
        }
        Value::Object(result)
    }

    fn generate_string(&self, token: Option<crate::string::Token>) -> Value {
        Value::String(random_string(
            &token.unwrap_or_else(templates::tokens_hello_world),
        ))
    }

    fn generate_number(&mut self, min: Option<f64>, max: Option<f64>) -> Value {
        if rand::thread_rng().gen_bool(0.5) {
            self.generate_integer(min.map(|v| v as i64), max.map(|v| v as i64))
        } else {
            self.generate_float(min, max)
        }
    }

    fn generate_integer(&self, min: Option<i64>, max: Option<i64>) -> Value {
        let min = min.unwrap_or(-9_007_199_254_740_991);
        let max = max.unwrap_or(9_007_199_254_740_991);
        Value::Number(Number::from(int(min, max)))
    }

    fn generate_int64(&self, min: Option<i64>, max: Option<i64>) -> Value {
        let min = min.unwrap_or(i64::MIN);
        let max = max.unwrap_or(i64::MAX);
        Value::Number(Number::from(int64(min, max)))
    }

    fn generate_float(&self, min: Option<f64>, max: Option<f64>) -> Value {
        let min = min.unwrap_or(-f64::MAX);
        let max = max.unwrap_or(f64::MAX);
        let mut float = rand::thread_rng().gen::<f64>() * (max - min) + min;
        float = float.max(min).min(max);
        Number::from_f64(float)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }

    fn generate_boolean(&self, value: Option<bool>) -> Value {
        Value::Bool(value.unwrap_or_else(|| rand::thread_rng().gen_bool(0.5)))
    }

    fn generate_bin(
        &self,
        min: Option<usize>,
        max: Option<usize>,
        omin: Option<u8>,
        omax: Option<u8>,
    ) -> Value {
        let min = min.unwrap_or(0);
        let max = max.unwrap_or(5);
        let omin = omin.unwrap_or(0);
        let omax = omax.unwrap_or(255);
        let length = self.minmax(min, max);

        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            bytes.push(Value::Number(Number::from(int(omin as i64, omax as i64))));
        }
        Value::Array(bytes)
    }

    fn generate_literal(&self, value: Value) -> Value {
        clone_json(&value)
    }

    fn generate_or(&mut self, options: Vec<Template>) -> Value {
        if options.is_empty() {
            return Value::Null;
        }
        let idx = int(0, options.len() as i64 - 1) as usize;
        self.generate(options[idx].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_default_string_template() {
        let value = TemplateJson::gen(Some(Template::str(None)), None);
        assert!(value.is_string());
    }

    #[test]
    fn generates_object_with_required_fields() {
        let tpl = Template::obj(vec![
            ObjectTemplateField::required_literal_key("id", Template::int(Some(1), Some(10))),
            ObjectTemplateField::required_literal_key("name", Template::str(None)),
        ]);
        let value = TemplateJson::gen(Some(tpl), None);
        let obj = value.as_object().expect("object");
        assert!(obj.get("id").is_some());
        assert!(obj.get("name").is_some());
    }

    #[test]
    fn optional_field_can_be_omitted() {
        let tpl = Template::obj(vec![ObjectTemplateField::optional_literal_key(
            "nickname",
            Template::str(None),
            1.0,
        )]);
        let value = TemplateJson::gen(Some(tpl), None);
        let obj = value.as_object().expect("object");
        assert!(obj.get("nickname").is_none());
    }
}
