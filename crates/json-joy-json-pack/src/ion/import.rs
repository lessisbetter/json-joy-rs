//! Ion symbol table import chain.
//!
//! Upstream reference: `json-pack/src/ion/Import.ts`

use std::collections::HashMap;

use super::constants::SYSTEM_SYMBOLS;
use super::types::SymbolTable;

/// Layered symbol table import (parent + local symbol entries).
#[derive(Debug, Clone)]
pub struct Import {
    parent: Option<Box<Import>>,
    pub symbols: SymbolTable,
    pub offset: usize,
    pub length: usize,
    by_text: HashMap<String, usize>,
}

impl Import {
    /// Create a new import layer with an optional parent import.
    pub fn new(parent: Option<Import>, symbols: SymbolTable) -> Self {
        let offset = parent
            .as_ref()
            .map(|p| p.offset + p.length)
            .unwrap_or(1usize);
        let length = symbols.len();
        let mut by_text = HashMap::with_capacity(length);
        for (i, symbol) in symbols.iter().enumerate() {
            by_text.insert(symbol.clone(), offset + i);
        }
        Self {
            parent: parent.map(Box::new),
            symbols,
            offset,
            length,
            by_text,
        }
    }

    /// Get symbol ID by text.
    pub fn get_id(&self, symbol: &str) -> Option<usize> {
        if let Some(id) = self.by_text.get(symbol) {
            return Some(*id);
        }
        self.parent
            .as_ref()
            .and_then(|parent| parent.get_id(symbol))
    }

    /// Get symbol text by ID.
    pub fn get_text(&self, id: usize) -> Option<&str> {
        if id < self.offset {
            return self.parent.as_ref().and_then(|parent| parent.get_text(id));
        }
        self.symbols
            .get(id.saturating_sub(self.offset))
            .map(String::as_str)
    }

    /// Add a symbol to this import layer and return its symbol ID.
    pub fn add(&mut self, symbol: &str) -> usize {
        if let Some(id) = self.by_text.get(symbol) {
            return *id;
        }
        let id = self.offset + self.symbols.len();
        self.symbols.push(symbol.to_owned());
        self.length += 1;
        self.by_text.insert(symbol.to_owned(), id);
        id
    }
}

/// Upstream-equivalent system symbol table (`$ion`..`$ion_shared_symbol_table`).
pub fn system_symbol_table() -> SymbolTable {
    SYSTEM_SYMBOLS
        .iter()
        .skip(1)
        .map(|s| (*s).to_owned())
        .collect()
}

/// Build a root import seeded with system symbols.
pub fn system_symbol_import() -> Import {
    Import::new(None, system_symbol_table())
}
