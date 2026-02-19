//! Converts TypeScript AST nodes to source text.
//!
//! Upstream reference: json-type/src/typescript/toText.ts

use super::types::{TsDeclaration, TsMember, TsParam, TsType};

const TAB: &str = "  ";

fn format_comment(comment: &str, indent: &str) -> String {
    let lines: Vec<&str> = comment.lines().collect();
    let mut out = format!("{}/**\n", indent);
    for line in &lines {
        out.push_str(&format!("{} * {}\n", indent, line));
    }
    out.push_str(&format!("{} */\n", indent));
    out
}

fn is_simple_type(t: &TsType) -> bool {
    matches!(
        t,
        TsType::Any
            | TsType::Boolean
            | TsType::Number
            | TsType::String
            | TsType::Null
            | TsType::Object
            | TsType::Unknown
            | TsType::TypeReference { .. }
    )
}

fn needs_quotes(name: &str) -> bool {
    name.is_empty()
        || name
            .chars()
            .any(|c| !c.is_alphanumeric() && c != '_' && c != '$')
        || name
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
}

fn normalize_key(name: &str) -> String {
    if needs_quotes(name) {
        format!("\"{}\"", name.replace('"', "\\\""))
    } else {
        name.to_string()
    }
}

/// Convert a `TsType` to TypeScript source text.
pub fn ts_type_to_text(t: &TsType, indent: &str) -> String {
    match t {
        TsType::Any => "any".into(),
        TsType::Boolean => "boolean".into(),
        TsType::Number => "number".into(),
        TsType::String => "string".into(),
        TsType::Null => "null".into(),
        TsType::Object => "object".into(),
        TsType::Unknown => "unknown".into(),
        TsType::True => "true".into(),
        TsType::False => "false".into(),
        TsType::StringLiteral(s) => {
            // JSON-encode the string to produce a valid TS string literal
            format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
        }
        TsType::NumericLiteral(n) => n.clone(),
        TsType::Array(elem) => {
            let inner = ts_type_to_text(elem, indent);
            if is_simple_type(elem) {
                format!("{}[]", inner)
            } else {
                format!("Array<{}>", inner)
            }
        }
        TsType::Tuple(elements) => {
            let has_complex = elements
                .iter()
                .any(|e| matches!(e, TsType::TypeLiteral { .. }));
            if has_complex {
                let inner_indent = format!("{}{}", indent, TAB);
                let parts: Vec<String> = elements
                    .iter()
                    .map(|e| format!("{}{}", inner_indent, ts_type_to_text(e, &inner_indent)))
                    .collect();
                format!("[\n{}\n{}]", parts.join(",\n"), indent)
            } else {
                let parts: Vec<String> = elements
                    .iter()
                    .map(|e| ts_type_to_text(e, indent))
                    .collect();
                format!("[{}]", parts.join(", "))
            }
        }
        TsType::Rest(inner) => format!("...{}", ts_type_to_text(inner, indent)),
        TsType::TypeLiteral { members, comment } => {
            if members.is_empty() {
                return "{}".into();
            }
            let inner_indent = format!("{}{}", indent, TAB);
            let mut out = String::new();
            if let Some(c) = comment {
                out.push_str(&format_comment(c, indent));
            }
            out.push_str("{\n");
            for member in members {
                out.push_str(&member_to_text(member, &inner_indent));
            }
            out.push_str(&format!("{}}}", indent));
            out
        }
        TsType::Union(types) => {
            let parts: Vec<String> = types.iter().map(|t| ts_type_to_text(t, indent)).collect();
            parts.join(" | ")
        }
        TsType::TypeReference { name, type_args } => {
            if type_args.is_empty() {
                name.clone()
            } else {
                let args: Vec<String> = type_args
                    .iter()
                    .map(|a| ts_type_to_text(a, indent))
                    .collect();
                format!("{}<{}>", name, args.join(", "))
            }
        }
        TsType::FnType {
            params,
            return_type,
        } => {
            let param_strs: Vec<String> = params.iter().map(|p| param_to_text(p, indent)).collect();
            format!(
                "({}) => {}",
                param_strs.join(", "),
                ts_type_to_text(return_type, indent)
            )
        }
    }
}

fn param_to_text(p: &TsParam, indent: &str) -> String {
    format!("{}: {}", p.name, ts_type_to_text(&p.type_, indent))
}

fn member_to_text(member: &TsMember, indent: &str) -> String {
    match member {
        TsMember::Property {
            name,
            type_,
            optional,
            comment,
        } => {
            let mut out = String::new();
            if let Some(c) = comment {
                out.push_str(&format_comment(c, indent));
            }
            let opt = if *optional { "?" } else { "" };
            let key = normalize_key(name);
            out.push_str(&format!(
                "{}{}{}: {};\n",
                indent,
                key,
                opt,
                ts_type_to_text(type_, indent)
            ));
            out
        }
        TsMember::Index { type_ } => {
            format!(
                "{}[key: string]: {};\n",
                indent,
                ts_type_to_text(type_, indent)
            )
        }
    }
}

/// Convert a top-level `TsDeclaration` to TypeScript source text.
pub fn declaration_to_text(decl: &TsDeclaration, indent: &str) -> String {
    match decl {
        TsDeclaration::Interface {
            name,
            members,
            comment,
        } => {
            let inner_indent = format!("{}{}", indent, TAB);
            let mut out = String::new();
            if let Some(c) = comment {
                out.push_str(&format_comment(c, indent));
            }
            out.push_str(&format!("{}export interface {} {{\n", indent, name));
            for member in members {
                out.push_str(&member_to_text(member, &inner_indent));
            }
            out.push_str(&format!("{}}}\n", indent));
            out
        }
        TsDeclaration::TypeAlias {
            name,
            type_,
            comment,
        } => {
            let mut out = String::new();
            if let Some(c) = comment {
                out.push_str(&format_comment(c, indent));
            }
            out.push_str(&format!(
                "{}export type {} = {};\n",
                indent,
                name,
                ts_type_to_text(type_, indent)
            ));
            out
        }
        TsDeclaration::Module { name, statements } => {
            let inner_indent = format!("{}{}", indent, TAB);
            let mut out = format!("{}export namespace {} {{\n", indent, name);
            for stmt in statements {
                out.push_str(&declaration_to_text(stmt, &inner_indent));
            }
            out.push_str(&format!("{}}}\n", indent));
            out
        }
    }
}

/// Convert a `TsType` or `TsDeclaration` to TypeScript source text.
///
/// Ports `toText` from `json-type/src/typescript/toText.ts`.
pub fn to_text(type_: &TsType) -> String {
    ts_type_to_text(type_, "")
}
