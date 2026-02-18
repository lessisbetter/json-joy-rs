//! Binary encoding utilities for the JSON CRDT Patch protocol.

pub mod crdt_writer;
pub mod crdt_reader;

pub use crdt_writer::CrdtWriter;
pub use crdt_reader::CrdtReader;
