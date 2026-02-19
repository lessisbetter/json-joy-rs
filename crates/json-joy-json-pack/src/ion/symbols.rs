//! Ion symbol table management.
//!
//! Upstream reference: `json-pack/src/ion/Import.ts`, `json-pack/src/ion/symbols.ts`

use std::collections::HashMap;

use super::constants::SYSTEM_SYMBOLS;

/// Ion symbol table (layered: child → parent).
///
/// Symbols are 1-indexed. System symbols occupy IDs 1..=9.
/// User-defined symbols follow, starting at ID 10.
#[derive(Debug, Clone)]
pub struct SymbolTable {
    /// Offset of the first symbol in this layer (1-based).
    pub offset: u32,
    /// Symbols owned by this layer (parallel to the IDs starting at `offset`).
    symbols: Vec<String>,
    /// Reverse lookup: text → ID.
    lookup: HashMap<String, u32>,
}

impl SymbolTable {
    /// Creates the root system symbol table.
    pub fn system() -> Self {
        let mut lookup = HashMap::new();
        let mut symbols = Vec::new();
        // System symbols are 1-indexed; slot 0 is unused.
        for (i, &sym) in SYSTEM_SYMBOLS.iter().enumerate() {
            if i == 0 {
                continue; // skip slot 0
            }
            lookup.insert(sym.to_string(), i as u32);
            symbols.push(sym.to_string());
        }
        Self {
            offset: 1,
            symbols,
            lookup,
        }
    }

    /// Creates a user symbol table layered on top of `parent`.
    pub fn user(parent_end: u32) -> Self {
        Self {
            offset: parent_end + 1,
            symbols: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Looks up a symbol text and returns its ID, or `None` if not found.
    pub fn get_id(&self, text: &str) -> Option<u32> {
        self.lookup.get(text).copied()
    }

    /// Returns the text for a given symbol ID, or `None` if out of range.
    pub fn get_text(&self, id: u32) -> Option<&str> {
        if id < self.offset {
            return None;
        }
        let idx = (id - self.offset) as usize;
        self.symbols.get(idx).map(|s| s.as_str())
    }

    /// Returns the last symbol ID owned by this table.
    ///
    /// Precondition: `symbols` must be non-empty. If empty, returns the ID just
    /// before this table's range (i.e., `offset - 1`), which callers use to
    /// compute the next layer's offset via `SymbolTable::user(end)`.
    pub fn end(&self) -> u32 {
        debug_assert!(
            !self.symbols.is_empty(),
            "end() called on empty SymbolTable"
        );
        self.offset + self.symbols.len() as u32 - 1
    }

    /// Adds a symbol text. Returns existing ID if already present.
    pub fn add(&mut self, text: &str) -> u32 {
        if let Some(&id) = self.lookup.get(text) {
            return id;
        }
        let id = self.offset + self.symbols.len() as u32;
        self.symbols.push(text.to_string());
        self.lookup.insert(text.to_string(), id);
        id
    }

    /// Returns all user-added symbol texts (in order, excluding system symbols).
    pub fn user_symbols(&self) -> &[String] {
        &self.symbols
    }
}

/// Combined symbol context: system table + user table.
#[derive(Debug)]
pub struct IonSymbols {
    pub system: SymbolTable,
    pub user: SymbolTable,
}

impl IonSymbols {
    pub fn new() -> Self {
        let system = SymbolTable::system();
        let system_end = system.end();
        Self {
            system,
            user: SymbolTable::user(system_end),
        }
    }

    pub fn get_id(&self, text: &str) -> Option<u32> {
        self.system.get_id(text).or_else(|| self.user.get_id(text))
    }

    pub fn get_text(&self, id: u32) -> Option<&str> {
        self.system.get_text(id).or_else(|| self.user.get_text(id))
    }

    pub fn add(&mut self, text: &str) -> u32 {
        if let Some(id) = self.system.get_id(text) {
            return id;
        }
        self.user.add(text)
    }

    pub fn has_user_symbols(&self) -> bool {
        !self.user.user_symbols().is_empty()
    }

    pub fn user_symbols(&self) -> &[String] {
        self.user.user_symbols()
    }
}

impl Default for IonSymbols {
    fn default() -> Self {
        Self::new()
    }
}
