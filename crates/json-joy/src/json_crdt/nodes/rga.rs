//! Simplified RGA (Replicated Growable Array) implementation.
//!
//! Mirrors the core semantics of `AbstractRga.ts` but uses a simple
//! `Vec<Chunk<T>>` instead of the balanced binary search tree in the
//! upstream. This gives O(n) per operation instead of O(log n), which
//! is correct but not optimised for large documents.

use crate::json_crdt_patch::clock::{Ts, Tss, compare};

// ── ChunkData ─────────────────────────────────────────────────────────────

/// Trait for chunk payload types that can be split at a logical item offset.
///
/// Required for partial-chunk deletion: when a deletion range covers only
/// part of a chunk, the chunk must be split before the covered part is
/// marked deleted.
pub trait ChunkData: Clone {
    /// Split `self` at logical offset `at` (number of items before the split).
    /// Modifies `self` to hold items `[0, at)` and returns items `[at, len)`.
    fn split_at_offset(&mut self, at: usize) -> Self;
}

impl ChunkData for String {
    fn split_at_offset(&mut self, at: usize) -> Self {
        // Locate the byte position of the `at`-th character.
        let byte_pos = self.char_indices().nth(at).map(|(i, _)| i).unwrap_or(self.len());
        // `String::split_off` mutates self to hold the prefix and returns suffix.
        self.split_off(byte_pos)
    }
}

impl ChunkData for Vec<u8> {
    fn split_at_offset(&mut self, at: usize) -> Self {
        self.split_off(at)
    }
}

impl ChunkData for Vec<Ts> {
    fn split_at_offset(&mut self, at: usize) -> Self {
        self.split_off(at)
    }
}

// ── Chunk ─────────────────────────────────────────────────────────────────

/// One chunk in the RGA sequence.
///
/// A chunk represents a contiguous run of items all inserted by the same
/// operation.  Items within a chunk always carry consecutive timestamps
/// `id, id+1, id+2, ...`.
#[derive(Debug, Clone)]
pub struct Chunk<T: Clone> {
    /// Timestamp of the *first* item in this chunk.
    pub id: Ts,
    /// Number of logical items in this chunk (including deleted ones).
    pub span: u64,
    /// Whether all items in this chunk are deleted.
    pub deleted: bool,
    /// The actual content.  `None` if the chunk is a deleted tombstone.
    pub data: Option<T>,
}

impl<T: Clone> Chunk<T> {
    pub fn new(id: Ts, span: u64, data: T) -> Self {
        Self { id, span, deleted: false, data: Some(data) }
    }

    pub fn len(&self) -> u64 {
        if self.deleted { 0 } else { self.span }
    }
}

// ── Rga ───────────────────────────────────────────────────────────────────

/// A simple linear-scan RGA sequence.
#[derive(Debug, Clone, Default)]
pub struct Rga<T: Clone> {
    pub chunks: Vec<Chunk<T>>,
}

impl<T: Clone + ChunkData> Rga<T> {
    pub fn new() -> Self {
        Self { chunks: Vec::new() }
    }

    /// Find the chunk index whose ID range contains `ts`, or `None`.
    pub fn find_by_id(&self, ts: Ts) -> Option<usize> {
        self.chunks.iter().position(|c| {
            c.id.sid == ts.sid && c.id.time <= ts.time && ts.time < c.id.time + c.span
        })
    }

    /// Insert `data` with timestamp `id` (span = data length) after the
    /// specific item identified by `after`.  If `after` is the ORIGIN
    /// sentinel `(0, 0)`, insert at the beginning.
    ///
    /// When `after` falls in the middle of a multi-item chunk the chunk is
    /// split so the insertion lands immediately after the targeted item.
    ///
    /// Concurrent inserts at the same position are ordered by `compare(id, existing)`.
    pub fn insert(&mut self, after: Ts, id: Ts, span: u64, data: T) {
        // Find the insertion point: right after the specific item `after`.
        let insert_pos = if after.sid == 0 && after.time == 0 {
            0  // ORIGIN → prepend
        } else {
            match self.find_by_id(after) {
                Some(idx) => {
                    // If `after` is not the last item in the chunk, split the
                    // chunk so that the insertion point is correct.
                    let chunk_last_time = self.chunks[idx].id.time + self.chunks[idx].span - 1;
                    if after.time < chunk_last_time {
                        let split_offset = (after.time - self.chunks[idx].id.time + 1) as usize;
                        self.split_chunk_at(idx, split_offset);
                    }
                    idx + 1
                }
                None => self.chunks.len(),
            }
        };

        // Among concurrent inserts at the same position, a chunk with a higher
        // timestamp has priority and goes further right.
        let mut pos = insert_pos;
        while pos < self.chunks.len() {
            let existing = &self.chunks[pos];
            if compare(existing.id, id) > 0 {
                pos += 1;
            } else {
                break;
            }
        }

        self.chunks.insert(pos, Chunk::new(id, span, data));
    }

    // ── Chunk splitting ──────────────────────────────────────────────────

    /// Split the chunk at `chunk_idx` at logical offset `at_offset`.
    ///
    /// After the call:
    /// - `chunks[chunk_idx]` holds items `[0, at_offset)`.
    /// - `chunks[chunk_idx + 1]` holds items `[at_offset, original_span)`.
    fn split_chunk_at(&mut self, chunk_idx: usize, at_offset: usize) {
        if at_offset == 0 { return; }
        let span = self.chunks[chunk_idx].span;
        if at_offset as u64 >= span { return; }

        let chunk = &mut self.chunks[chunk_idx];
        let id = chunk.id;
        let deleted = chunk.deleted;

        // `Option::map` returns `None` for already-deleted tombstones; both
        // halves remain as deleted chunks, which is the correct behavior.
        let right_data = chunk.data.as_mut().map(|d| d.split_at_offset(at_offset));

        let right_chunk = Chunk {
            id: Ts::new(id.sid, id.time + at_offset as u64),
            span: span - at_offset as u64,
            deleted,
            data: right_data,
        };

        self.chunks[chunk_idx].span = at_offset as u64;
        self.chunks.insert(chunk_idx + 1, right_chunk);
    }

    // ── Deletion ─────────────────────────────────────────────────────────

    /// Delete all items covered by the given timestamp spans.
    ///
    /// Chunks that are only partially covered by a deletion span are split
    /// at the deletion boundaries so that only the targeted items are removed.
    pub fn delete(&mut self, spans: &[Tss]) {
        for tss in spans {
            let del_start = tss.time;
            let del_end = tss.time + tss.span; // exclusive upper bound
            let sid = tss.sid;

            let mut i = 0;
            while i < self.chunks.len() {
                let chunk = &self.chunks[i];

                // Skip chunks from different sessions.
                if chunk.id.sid != sid {
                    i += 1;
                    continue;
                }

                let chunk_start = chunk.id.time;
                let chunk_end = chunk.id.time + chunk.span;

                // No overlap.
                if chunk_start >= del_end || chunk_end <= del_start {
                    i += 1;
                    continue;
                }

                // Compute overlap: [overlap_start, overlap_end).
                let overlap_start = del_start.max(chunk_start);
                let overlap_end   = del_end.min(chunk_end);

                // Split off the prefix that precedes the deletion (if any).
                if overlap_start > chunk_start {
                    let prefix_len = (overlap_start - chunk_start) as usize;
                    self.split_chunk_at(i, prefix_len);
                    i += 1; // advance to the right half (starts at overlap_start)
                }

                // Split off the suffix that follows the deletion (if any).
                let chunk = &self.chunks[i];
                let chunk_end = chunk.id.time + chunk.span;
                if overlap_end < chunk_end {
                    let del_len = (overlap_end - self.chunks[i].id.time) as usize;
                    self.split_chunk_at(i, del_len);
                    // chunks[i] now covers exactly [overlap_start, overlap_end)
                }

                // Mark the overlapping chunk as deleted.
                let chunk = &mut self.chunks[i];
                chunk.deleted = true;
                chunk.data = None;

                i += 1;
            }
        }
    }

    // ── Iteration ────────────────────────────────────────────────────────

    /// Iterate live (non-deleted) chunks.
    pub fn iter_live(&self) -> impl Iterator<Item = &Chunk<T>> {
        self.chunks.iter().filter(|c| !c.deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::{ts, tss};

    fn origin() -> Ts { ts(0, 0) }
    fn sid() -> u64 { 1 }

    #[test]
    fn insert_single_chunk() {
        let mut rga: Rga<String> = Rga::new();
        rga.insert(origin(), ts(sid(), 1), 5, "hello".to_string());
        assert_eq!(rga.chunks.len(), 1);
        assert_eq!(rga.chunks[0].data.as_deref(), Some("hello"));
    }

    #[test]
    fn view_after_insert() {
        let mut rga: Rga<String> = Rga::new();
        rga.insert(origin(), ts(sid(), 1), 5, "hello".to_string());
        let s: String = rga.iter_live().filter_map(|c| c.data.as_deref()).collect();
        assert_eq!(s, "hello");
    }

    #[test]
    fn partial_delete_middle() {
        let mut rga: Rga<String> = Rga::new();
        // Insert "hello" at ts(1,1), span=5 → items at times 1,2,3,4,5
        rga.insert(origin(), ts(1, 1), 5, "hello".to_string());
        // Delete 'e','l','l' = tss(1, 2, 3) → times 2,3,4
        rga.delete(&[tss(1, 2, 3)]);

        let s: String = rga.iter_live().filter_map(|c| c.data.as_deref()).collect();
        assert_eq!(s, "ho");
    }

    #[test]
    fn partial_delete_prefix() {
        let mut rga: Rga<String> = Rga::new();
        rga.insert(origin(), ts(1, 1), 5, "hello".to_string());
        // Delete 'h','e' = tss(1, 1, 2)
        rga.delete(&[tss(1, 1, 2)]);
        let s: String = rga.iter_live().filter_map(|c| c.data.as_deref()).collect();
        assert_eq!(s, "llo");
    }

    #[test]
    fn partial_delete_suffix() {
        let mut rga: Rga<String> = Rga::new();
        rga.insert(origin(), ts(1, 1), 5, "hello".to_string());
        // Delete 'l','l','o' = tss(1, 3, 3)
        rga.delete(&[tss(1, 3, 3)]);
        let s: String = rga.iter_live().filter_map(|c| c.data.as_deref()).collect();
        assert_eq!(s, "he");
    }

    #[test]
    fn delete_full_chunk() {
        let mut rga: Rga<String> = Rga::new();
        rga.insert(origin(), ts(1, 1), 5, "hello".to_string());
        rga.delete(&[tss(1, 1, 5)]);
        let s: String = rga.iter_live().filter_map(|c| c.data.as_deref()).collect();
        assert_eq!(s, "");
    }

    #[test]
    fn two_chunk_delete_spanning_boundary() {
        let mut rga: Rga<String> = Rga::new();
        // "he" at ts(1,1), "llo" at ts(1,3) inserted after chunk 1
        rga.insert(origin(),   ts(1, 1), 2, "he".to_string());
        rga.insert(ts(1, 2),   ts(1, 3), 3, "llo".to_string());
        // Delete 'e','l' spanning both chunks = tss(1, 2, 2)
        rga.delete(&[tss(1, 2, 2)]);
        let s: String = rga.iter_live().filter_map(|c| c.data.as_deref()).collect();
        assert_eq!(s, "hlo");
    }
}
