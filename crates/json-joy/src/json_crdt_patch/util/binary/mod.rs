//! Binary encoding utilities for the JSON CRDT Patch protocol.

pub mod crdt_reader;
pub mod crdt_writer;

pub use crdt_reader::CrdtReader;
pub use crdt_writer::CrdtWriter;
