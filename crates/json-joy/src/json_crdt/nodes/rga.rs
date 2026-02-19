//! RGA (Replicated Growable Array) — dual splay-tree implementation.
//!
//! Mirrors `AbstractRga.ts` from upstream `json-joy@17.67.0`.
//!
//! The RGA maintains two concurrent splay trees over an arena (`Vec<Chunk<T>>`):
//!
//! - **Position tree** (`p` / `l` / `r`) — BST ordered by document position,
//!   aggregating `len` in each subtree for O(log n) `findChunk(pos)`.
//! - **ID tree** (`p2` / `l2` / `r2`) — BST ordered by `(sid, time)` for
//!   O(log n) `findById`.
//!
//! Chunks also carry a `s` (split-link) pointer that threads together
//! consecutive pieces of the same original insertion operation.

use crate::json_crdt_patch::clock::{compare, Ts, Tss};
use sonic_forest::{Node, Node2};

// ── ChunkData ─────────────────────────────────────────────────────────────

/// Trait for chunk payload types that can be split and merged at logical item offsets.
pub trait ChunkData: Clone {
    /// Split `self` at logical offset `at` (items before the split).
    /// Modifies `self` to hold items `[0, at)` and returns items `[at, len)`.
    fn split_at_offset(&mut self, at: usize) -> Self;
    /// Append `other` to `self` (the inverse of `split_at_offset`).
    /// Mirrors `Chunk.merge(content)` in the upstream TypeScript.
    fn merge(&mut self, other: Self);
}

impl ChunkData for String {
    fn split_at_offset(&mut self, at: usize) -> Self {
        let byte_pos = self
            .char_indices()
            .nth(at)
            .map(|(i, _)| i)
            .unwrap_or(self.len());
        self.split_off(byte_pos)
    }
    fn merge(&mut self, other: Self) {
        self.push_str(&other);
    }
}

impl ChunkData for Vec<u8> {
    fn split_at_offset(&mut self, at: usize) -> Self {
        self.split_off(at)
    }
    fn merge(&mut self, other: Self) {
        self.extend(other);
    }
}

impl ChunkData for Vec<Ts> {
    fn split_at_offset(&mut self, at: usize) -> Self {
        self.split_off(at)
    }
    fn merge(&mut self, other: Self) {
        self.extend(other);
    }
}

// ── Chunk ─────────────────────────────────────────────────────────────────

/// One chunk in the RGA sequence.
#[derive(Debug, Clone)]
pub struct Chunk<T: Clone> {
    /// Timestamp of the *first* item in this chunk.
    pub id: Ts,
    /// Number of logical items in this chunk (includes deleted ones).
    pub span: u64,
    /// Whether all items in this chunk are deleted (tombstone).
    pub deleted: bool,
    /// Actual content. `None` if the chunk is a deleted tombstone.
    pub data: Option<T>,
    /// Aggregated live (non-deleted) content length in this subtree.
    pub len: u64,
    // Position tree links
    pub p: Option<u32>,
    pub l: Option<u32>,
    pub r: Option<u32>,
    // ID tree links
    pub p2: Option<u32>,
    pub l2: Option<u32>,
    pub r2: Option<u32>,
    /// Split link — next chunk split from this one.
    pub s: Option<u32>,
}

impl<T: Clone> Chunk<T> {
    pub fn new(id: Ts, span: u64, data: T) -> Self {
        Self {
            id,
            span,
            deleted: false,
            data: Some(data),
            len: span,
            p: None,
            l: None,
            r: None,
            p2: None,
            l2: None,
            r2: None,
            s: None,
        }
    }

    pub fn new_deleted(id: Ts, span: u64) -> Self {
        Self {
            id,
            span,
            deleted: true,
            data: None,
            len: 0,
            p: None,
            l: None,
            r: None,
            p2: None,
            l2: None,
            r2: None,
            s: None,
        }
    }

    /// Live length of this chunk (0 if deleted, else span).
    pub fn len(&self) -> u64 {
        if self.deleted {
            0
        } else {
            self.span
        }
    }
}

impl<T: Clone> Node for Chunk<T> {
    fn p(&self) -> Option<u32> {
        self.p
    }
    fn l(&self) -> Option<u32> {
        self.l
    }
    fn r(&self) -> Option<u32> {
        self.r
    }
    fn set_p(&mut self, v: Option<u32>) {
        self.p = v;
    }
    fn set_l(&mut self, v: Option<u32>) {
        self.l = v;
    }
    fn set_r(&mut self, v: Option<u32>) {
        self.r = v;
    }
}

impl<T: Clone> Node2 for Chunk<T> {
    fn p2(&self) -> Option<u32> {
        self.p2
    }
    fn l2(&self) -> Option<u32> {
        self.l2
    }
    fn r2(&self) -> Option<u32> {
        self.r2
    }
    fn set_p2(&mut self, v: Option<u32>) {
        self.p2 = v;
    }
    fn set_l2(&mut self, v: Option<u32>) {
        self.l2 = v;
    }
    fn set_r2(&mut self, v: Option<u32>) {
        self.r2 = v;
    }
}

// ── Rga ───────────────────────────────────────────────────────────────────

/// RGA sequence backed by an arena-allocated dual splay tree.
#[derive(Debug, Clone)]
pub struct Rga<T: Clone> {
    /// Arena — chunk at index `i` is `chunks[i]`.
    pub chunks: Vec<Chunk<T>>,
    /// Root of the position tree.
    pub root: Option<u32>,
    /// Root of the ID tree.
    pub ids: Option<u32>,
    /// Total chunk count (including tombstones).
    pub count: usize,
}

// ── len aggregation helpers ───────────────────────────────────────────────

/// Recalculate `chunk.len = (del?0:span) + l.len + r.len`.
fn update_len_one<T: Clone>(chunks: &mut Vec<Chunk<T>>, idx: u32) {
    let c = &chunks[idx as usize];
    let l_len = c.l.map(|l| chunks[l as usize].len).unwrap_or(0);
    let r_len = c.r.map(|r| chunks[r as usize].len).unwrap_or(0);
    let span = if c.deleted { 0 } else { c.span };
    chunks[idx as usize].len = span + l_len + r_len;
}

/// Same as `update_len_one` but always uses `span` (chunk is known live).
fn update_len_one_live<T: Clone>(chunks: &mut Vec<Chunk<T>>, idx: u32) {
    let c = &chunks[idx as usize];
    let l_len = c.l.map(|l| chunks[l as usize].len).unwrap_or(0);
    let r_len = c.r.map(|r| chunks[r as usize].len).unwrap_or(0);
    chunks[idx as usize].len = c.span + l_len + r_len;
}

/// Propagate a `delta` up the position tree from `idx` to the root.
fn d_len<T: Clone>(chunks: &mut Vec<Chunk<T>>, mut idx: Option<u32>, delta: i64) {
    while let Some(i) = idx {
        let c = &mut chunks[i as usize];
        c.len = (c.len as i64 + delta) as u64;
        idx = c.p;
    }
}

// ── position-tree splay ───────────────────────────────────────────────────

/// Splay `idx` to the root of the position tree, updating `len` aggregates.
/// Mirrors `AbstractRga.splay()`.
fn splay_pos<T: Clone>(chunks: &mut Vec<Chunk<T>>, root: &mut Option<u32>, idx: u32) {
    loop {
        let p = chunks[idx as usize].p;
        let Some(p) = p else {
            break;
        };
        let pp = chunks[p as usize].p;
        let l2 = chunks[p as usize].l == Some(idx);
        if let Some(pp) = pp {
            let l1 = chunks[pp as usize].l == Some(p);
            *root = match (l1, l2) {
                (true, true) => sonic_forest::ll_splay(chunks, *root, idx, p, pp),
                (true, false) => sonic_forest::lr_splay(chunks, *root, idx, p, pp),
                (false, true) => sonic_forest::rl_splay(chunks, *root, idx, p, pp),
                (false, false) => sonic_forest::rr_splay(chunks, *root, idx, p, pp),
            };
            update_len_one(chunks, pp);
            update_len_one(chunks, p);
            update_len_one_live(chunks, idx);
        } else {
            if l2 {
                sonic_forest::r_splay(chunks, idx, p);
            } else {
                sonic_forest::l_splay(chunks, idx, p);
            }
            *root = Some(idx);
            update_len_one(chunks, p);
            update_len_one_live(chunks, idx);
            break;
        }
    }
}

// ── position-tree traversal ───────────────────────────────────────────────

fn pos_next<T: Clone>(chunks: &[Chunk<T>], idx: u32) -> Option<u32> {
    if let Some(r) = chunks[idx as usize].r {
        let mut curr = r;
        while let Some(l) = chunks[curr as usize].l {
            curr = l;
        }
        return Some(curr);
    }
    let mut curr = idx;
    let mut p = chunks[idx as usize].p;
    while let Some(pi) = p {
        if chunks[pi as usize].r == Some(curr) {
            curr = pi;
            p = chunks[pi as usize].p;
        } else {
            return Some(pi);
        }
    }
    None
}

fn pos_prev<T: Clone>(chunks: &[Chunk<T>], idx: u32) -> Option<u32> {
    if let Some(l) = chunks[idx as usize].l {
        let mut curr = l;
        while let Some(r) = chunks[curr as usize].r {
            curr = r;
        }
        return Some(curr);
    }
    let mut curr = idx;
    let mut p = chunks[idx as usize].p;
    while let Some(pi) = p {
        if chunks[pi as usize].l == Some(curr) {
            curr = pi;
            p = chunks[pi as usize].p;
        } else {
            return Some(pi);
        }
    }
    None
}

fn pos_first<T: Clone>(chunks: &[Chunk<T>], root: Option<u32>) -> Option<u32> {
    let mut curr = root;
    while let Some(idx) = curr {
        match chunks[idx as usize].l {
            Some(l) => curr = Some(l),
            None => return Some(idx),
        }
    }
    None
}

// ── ID-tree operations ────────────────────────────────────────────────────

fn compare_by_id<T: Clone>(a: &Chunk<T>, b: &Chunk<T>) -> std::cmp::Ordering {
    a.id.sid.cmp(&b.id.sid).then(a.id.time.cmp(&b.id.time))
}

fn insert_id<T: Clone>(rga: &mut Rga<T>, idx: u32) {
    rga.ids = sonic_forest::insert2(&mut rga.chunks, rga.ids, idx, compare_by_id);
    rga.count += 1;
    rga.ids = sonic_forest::splay2(&mut rga.chunks, rga.ids, idx);
}

fn insert_id_fast<T: Clone>(rga: &mut Rga<T>, idx: u32) {
    rga.ids = sonic_forest::insert2(&mut rga.chunks, rga.ids, idx, compare_by_id);
    rga.count += 1;
}

fn delete_id<T: Clone>(rga: &mut Rga<T>, idx: u32) {
    rga.ids = sonic_forest::remove2(&mut rga.chunks, rga.ids, idx);
    rga.count -= 1;
}

// ── position-tree insertion primitives ───────────────────────────────────

/// Insert `idx` as the new left sub-tree of `before` in the position tree.
/// Mirrors `AbstractRga.insertBefore()`.
fn pos_insert_before<T: Clone>(rga: &mut Rga<T>, idx: u32, before: u32) {
    let l = rga.chunks[before as usize].l;
    rga.chunks[before as usize].l = Some(idx);
    rga.chunks[idx as usize].l = l;
    rga.chunks[idx as usize].p = Some(before);
    let l_len = if let Some(l) = l {
        rga.chunks[l as usize].p = Some(idx);
        rga.chunks[l as usize].len
    } else {
        0
    };
    let span = rga.chunks[idx as usize].span;
    rga.chunks[idx as usize].len = span + l_len;
    d_len(&mut rga.chunks, Some(before), span as i64);
    insert_id(rga, idx);
}

/// Insert `idx` as the new right child of `after` in the position tree.
/// Mirrors `AbstractRga.insertAfter()`.
fn pos_insert_after<T: Clone>(rga: &mut Rga<T>, idx: u32, after: u32) {
    let r = rga.chunks[after as usize].r;
    rga.chunks[after as usize].r = Some(idx);
    rga.chunks[idx as usize].r = r;
    rga.chunks[idx as usize].p = Some(after);
    let r_len = if let Some(r) = r {
        rga.chunks[r as usize].p = Some(idx);
        rga.chunks[r as usize].len
    } else {
        0
    };
    let span = rga.chunks[idx as usize].span;
    rga.chunks[idx as usize].len = span + r_len;
    d_len(&mut rga.chunks, Some(after), span as i64);
    insert_id(rga, idx);
}

/// Set `idx` as the position-tree root (first chunk ever).
fn set_root<T: Clone>(rga: &mut Rga<T>, idx: u32) {
    rga.root = Some(idx);
    insert_id(rga, idx);
}

// ── insertAfterRef ─────────────────────────────────────────────────────────

// ── mergeContent ─────────────────────────────────────────────────────────

/// Extend `left`'s data and span with `idx`'s data, then pop `idx` from the
/// arena.  Mirrors `AbstractRga.mergeContent()`.
fn merge_content<T: Clone + ChunkData>(rga: &mut Rga<T>, left: u32, idx: u32) {
    let span1 = rga.chunks[left as usize].span;
    let new_data = rga.chunks[idx as usize].data.take();
    let new_span = rga.chunks[idx as usize].span;
    if let (Some(ld), Some(nd)) = (rga.chunks[left as usize].data.as_mut(), new_data) {
        ld.merge(nd);
    }
    rga.chunks[left as usize].span += new_span;
    let delta = rga.chunks[left as usize].span - span1; // == new_span
    d_len(&mut rga.chunks, Some(left), delta as i64);
    rga.chunks[left as usize].s = None;
    // idx is left as an orphan in the arena (p = None, not in any tree).
    // The caller's subsequent splay_pos(idx) will see p = None and be a no-op,
    // mirroring how TypeScript lets the GC reclaim the discarded chunk while
    // splay(newChunk) returns immediately on !p.
}

// ── insertAfterRef ─────────────────────────────────────────────────────────

/// Insert `idx` after reference `ref_id`, scanning forward from `left`.
/// Mirrors `AbstractRga.insertAfterRef()`.
fn insert_after_ref<T: Clone + ChunkData>(rga: &mut Rga<T>, idx: u32, ref_id: Ts, mut left: u32) {
    let id = rga.chunks[idx as usize].id;
    let sid = id.sid;
    let time = id.time;
    let mut is_split = false;

    loop {
        let left_id = rga.chunks[left as usize].id;
        let left_next_tick = left_id.time + rga.chunks[left as usize].span;

        if rga.chunks[left as usize].s.is_none() {
            is_split =
                left_id.sid == sid && left_next_tick == time && left_next_tick - 1 == ref_id.time;
            if is_split {
                rga.chunks[left as usize].s = Some(idx);
            }
        }

        let right = pos_next(&rga.chunks, left);
        let Some(right) = right else {
            break;
        };

        let right_id = rga.chunks[right as usize].id;
        let right_id_time = right_id.time;
        let right_id_sid = right_id.sid;

        if right_id_time < time {
            break;
        }
        if right_id_time == time {
            if right_id_sid == sid {
                // Already exists — undo the split link we may have set and
                // leave idx as an orphan (p = None).  The caller's splay_pos
                // will be a no-op, matching upstream where splay(newChunk)
                // immediately returns on !newChunk.p.
                if is_split {
                    rga.chunks[left as usize].s = None;
                }
                return;
            }
            if right_id_sid < sid {
                break;
            }
        }
        left = right;
    }

    if is_split && !rga.chunks[left as usize].deleted {
        // Mirror AbstractRga.mergeContent(): fold the new chunk into `left`.
        merge_content(rga, left, idx);
    } else {
        pos_insert_after(rga, idx, left);
    }
}

// ── alloc_split_chunk ─────────────────────────────────────────────────────

/// Split the chunk at `idx` into `[0, ticks)` (left) and `[ticks, span)` (right).
/// Adds the right-half chunk to the arena but does NOT wire it into any tree.
/// Returns the arena index of the right-half chunk.
fn alloc_split_chunk<T: Clone + ChunkData>(rga: &mut Rga<T>, idx: u32, ticks: usize) -> u32 {
    let id = rga.chunks[idx as usize].id;
    let span = rga.chunks[idx as usize].span;
    let del = rga.chunks[idx as usize].deleted;

    let right_data = rga.chunks[idx as usize]
        .data
        .as_mut()
        .map(|d| d.split_at_offset(ticks));
    rga.chunks[idx as usize].span = ticks as u64;

    let right_id = Ts::new(id.sid, id.time + ticks as u64);
    let right_span = span - ticks as u64;

    let new_chunk = if del {
        Chunk::new_deleted(right_id, right_span)
    } else {
        match right_data {
            Some(d) => Chunk::new(right_id, right_span, d),
            None => Chunk::new_deleted(right_id, right_span),
        }
    };

    let new_idx = rga.chunks.len() as u32;
    rga.chunks.push(new_chunk);
    new_idx
}

// ── split_for_delete ──────────────────────────────────────────────────────

/// Split chunk at `idx` at `ticks`, wiring the right-half into the position
/// tree as the immediate right child of `idx`.  Also inserts the right-half
/// into the ID tree.  Mirrors `AbstractRga.split()`.
fn split_for_delete<T: Clone + ChunkData>(rga: &mut Rga<T>, idx: u32, ticks: usize) -> u32 {
    let s = rga.chunks[idx as usize].s;
    let at2 = alloc_split_chunk(rga, idx, ticks);
    let r = rga.chunks[idx as usize].r;

    rga.chunks[idx as usize].s = Some(at2);
    rga.chunks[at2 as usize].s = s;
    rga.chunks[at2 as usize].r = r;
    rga.chunks[idx as usize].r = Some(at2);
    rga.chunks[at2 as usize].p = Some(idx);
    if let Some(r) = r {
        rga.chunks[r as usize].p = Some(at2);
    }

    insert_id(rga, at2);
    at2
}

// ── insertInside ──────────────────────────────────────────────────────────

/// Insert `idx` inside chunk `at` at position `offset` within `at`.
/// Splits `at` into left `[0, offset)` and right `[offset, at.span)`,
/// places `idx` at the root of that subtree.
/// Mirrors `AbstractRga.insertInside()`.
fn insert_inside<T: Clone + ChunkData>(rga: &mut Rga<T>, idx: u32, at: u32, offset: usize) {
    // Snapshot pointers before we mutate anything.
    let p = rga.chunks[at as usize].p;
    let l = rga.chunks[at as usize].l;
    let r = rga.chunks[at as usize].r;
    let s = rga.chunks[at as usize].s;
    let len = rga.chunks[at as usize].len;

    // Split `at` into at (left) and at2 (right) — no tree wiring yet.
    let at2 = alloc_split_chunk(rga, at, offset);

    // Update split links.
    rga.chunks[at as usize].s = Some(at2);
    rga.chunks[at2 as usize].s = s;

    // Detach `at` and `at2` from any existing children (we re-wire below).
    rga.chunks[at as usize].l = None;
    rga.chunks[at as usize].r = None;
    rga.chunks[at2 as usize].l = None;
    rga.chunks[at2 as usize].r = None;

    // Wire idx into the position formerly occupied by `at`.
    rga.chunks[idx as usize].p = p;

    // Left side: l → [... → at]
    if l.is_none() {
        rga.chunks[idx as usize].l = Some(at);
        rga.chunks[at as usize].p = Some(idx);
    } else {
        let l = l.unwrap();
        rga.chunks[idx as usize].l = Some(l);
        rga.chunks[l as usize].p = Some(idx);
        // Attach `at` as right child of `l`, preserving l's right sub-tree.
        let a = rga.chunks[l as usize].r;
        rga.chunks[l as usize].r = Some(at);
        rga.chunks[at as usize].p = Some(l);
        rga.chunks[at as usize].l = a;
        if let Some(a) = a {
            rga.chunks[a as usize].p = Some(at);
        }
    }

    // Right side: [at2 → ...] → r
    if r.is_none() {
        rga.chunks[idx as usize].r = Some(at2);
        rga.chunks[at2 as usize].p = Some(idx);
    } else {
        let r = r.unwrap();
        rga.chunks[idx as usize].r = Some(r);
        rga.chunks[r as usize].p = Some(idx);
        // Attach `at2` as left child of `r`, preserving r's left sub-tree.
        let b = rga.chunks[r as usize].l;
        rga.chunks[r as usize].l = Some(at2);
        rga.chunks[at2 as usize].p = Some(r);
        rga.chunks[at2 as usize].r = b;
        if let Some(b) = b {
            rga.chunks[b as usize].p = Some(at2);
        }
    }

    // Wire idx into p's child slot.
    if p.is_none() {
        rga.root = Some(idx);
    } else {
        let p = p.unwrap();
        if rga.chunks[p as usize].l == Some(at) {
            rga.chunks[p as usize].l = Some(idx);
        } else {
            rga.chunks[p as usize].r = Some(idx);
        }
    }

    // Update len aggregates.
    update_len_one(&mut rga.chunks, at);
    update_len_one(&mut rga.chunks, at2);

    // Recalculate l and r's len (they each gained one extra child).
    if let Some(l) = l {
        let l_l_len = rga.chunks[l as usize]
            .l
            .map(|ll| rga.chunks[ll as usize].len)
            .unwrap_or(0);
        let at_len = rga.chunks[at as usize].len;
        let l_span = if rga.chunks[l as usize].deleted {
            0
        } else {
            rga.chunks[l as usize].span
        };
        rga.chunks[l as usize].len = l_l_len + at_len + l_span;
    }
    if let Some(r) = r {
        let r_r_len = rga.chunks[r as usize]
            .r
            .map(|rr| rga.chunks[rr as usize].len)
            .unwrap_or(0);
        let at2_len = rga.chunks[at2 as usize].len;
        let r_span = if rga.chunks[r as usize].deleted {
            0
        } else {
            rga.chunks[r as usize].span
        };
        rga.chunks[r as usize].len = r_r_len + at2_len + r_span;
    }

    // idx.len = original subtree len + idx's own span.
    let idx_span = rga.chunks[idx as usize].span;
    rga.chunks[idx as usize].len = len + idx_span;

    // Propagate idx's span up to the root.
    let mut curr = rga.chunks[idx as usize].p;
    while let Some(ci) = curr {
        rga.chunks[ci as usize].len += idx_span;
        curr = rga.chunks[ci as usize].p;
    }

    // Insert both new chunks into the ID tree.
    insert_id(rga, at2);
    insert_id_fast(rga, idx);
}

// ── insAfterRoot / insAfterChunk ──────────────────────────────────────────

/// Insert `idx` when the reference is the document root sentinel.
/// Mirrors `AbstractRga.insAfterRoot()`.
fn ins_after_root<T: Clone + ChunkData>(rga: &mut Rga<T>, after: Ts, idx: u32) {
    let first = pos_first(&rga.chunks, rga.root);
    if first.is_none() {
        set_root(rga, idx);
        return;
    }
    let first = first.unwrap();
    let cmp = compare(rga.chunks[first as usize].id, rga.chunks[idx as usize].id);
    if cmp < 0 {
        pos_insert_before(rga, idx, first);
    } else {
        // Check if first already contains this id.
        let f = &rga.chunks[first as usize];
        let id = rga.chunks[idx as usize].id;
        if f.id.sid == id.sid && f.id.time <= id.time && f.id.time + f.span > id.time {
            rga.chunks.pop(); // discard pre-allocated
            return;
        }
        insert_after_ref(rga, idx, after, first);
    }
}

/// Insert `idx` after the item at offset `chunk_offset` inside `chunk_idx`.
/// Mirrors `AbstractRga.insAfterChunk()`.
fn ins_after_chunk<T: Clone + ChunkData>(
    rga: &mut Rga<T>,
    after: Ts,
    chunk_idx: u32,
    chunk_offset: usize,
    idx: u32,
) {
    let at_id = rga.chunks[chunk_idx as usize].id;
    let at_span = rga.chunks[chunk_idx as usize].span;
    let new_id = rga.chunks[idx as usize].id;

    let needs_split = chunk_offset + 1 < at_span as usize;
    if needs_split {
        // Check if this id is already inside the chunk.
        if at_id.sid == new_id.sid
            && at_id.time <= new_id.time
            && at_id.time + at_span > new_id.time
        {
            rga.chunks.pop();
            return;
        }
        if new_id.time > after.time + 1 || new_id.sid > after.sid {
            insert_inside(rga, idx, chunk_idx, chunk_offset + 1);
            splay_pos(&mut rga.chunks, &mut rga.root, idx);
            return;
        }
    }
    insert_after_ref(rga, idx, after, chunk_idx);
    splay_pos(&mut rga.chunks, &mut rga.root, idx);
}

// ── deleteChunk ───────────────────────────────────────────────────────────

/// Remove chunk `idx` from both trees.  Mirrors `AbstractRga.deleteChunk()`.
fn delete_chunk<T: Clone>(rga: &mut Rga<T>, idx: u32) {
    delete_id(rga, idx);

    let p = rga.chunks[idx as usize].p;
    let l = rga.chunks[idx as usize].l;
    let r = rga.chunks[idx as usize].r;
    rga.chunks[idx as usize].p = None;
    rga.chunks[idx as usize].l = None;
    rga.chunks[idx as usize].r = None;

    match (l, r) {
        (None, None) => {
            if let Some(p) = p {
                if rga.chunks[p as usize].l == Some(idx) {
                    rga.chunks[p as usize].l = None;
                } else {
                    rga.chunks[p as usize].r = None;
                }
            } else {
                rga.root = None;
            }
        }
        (Some(l), Some(r)) => {
            let r_len = rga.chunks[r as usize].len;
            // Find rightmost descendant of l.
            let mut most_right = l;
            while let Some(mr) = rga.chunks[most_right as usize].r {
                most_right = mr;
            }
            rga.chunks[most_right as usize].r = Some(r);
            rga.chunks[r as usize].p = Some(most_right);

            if let Some(p) = p {
                if rga.chunks[p as usize].l == Some(idx) {
                    rga.chunks[p as usize].l = Some(l);
                } else {
                    rga.chunks[p as usize].r = Some(l);
                }
                rga.chunks[l as usize].p = Some(p);
            } else {
                rga.root = Some(l);
                rga.chunks[l as usize].p = None;
            }

            // Update len from most_right up to (but not including) p.
            let mut curr = Some(most_right);
            while curr != p {
                let ci = curr.unwrap();
                let cl = rga.chunks[ci as usize].l;
                let cr = rga.chunks[ci as usize].r;
                let cs = if rga.chunks[ci as usize].deleted {
                    0
                } else {
                    rga.chunks[ci as usize].span
                };
                let ll = cl.map(|l| rga.chunks[l as usize].len).unwrap_or(0);
                let rl = cr.map(|r| rga.chunks[r as usize].len).unwrap_or(0);
                rga.chunks[ci as usize].len = cs + ll + rl;
                curr = rga.chunks[ci as usize].p;
            }
            // Note: upstream uses a simpler `+= rLen` propagation; the above is equivalent.
            let _ = r_len; // suppress unused warning
        }
        _ => {
            let child = l.or(r).unwrap();
            rga.chunks[child as usize].p = p;
            if let Some(p) = p {
                if rga.chunks[p as usize].l == Some(idx) {
                    rga.chunks[p as usize].l = Some(child);
                } else {
                    rga.chunks[p as usize].r = Some(child);
                }
            } else {
                rga.root = Some(child);
            }
        }
    }
}

// ── mergeTombstones ───────────────────────────────────────────────────────

/// Attempt to merge two consecutive tombstones.
/// Returns `true` if merged.  Mirrors `AbstractRga.mergeTombstones()`.
fn merge_tombstones<T: Clone>(rga: &mut Rga<T>, ch1: u32, ch2: u32) -> bool {
    if !rga.chunks[ch1 as usize].deleted || !rga.chunks[ch2 as usize].deleted {
        return false;
    }
    let id1 = rga.chunks[ch1 as usize].id;
    let id2 = rga.chunks[ch2 as usize].id;
    if id1.sid != id2.sid {
        return false;
    }
    let ch1_span = rga.chunks[ch1 as usize].span;
    if id1.time + ch1_span != id2.time {
        return false;
    }
    let s2 = rga.chunks[ch2 as usize].s;
    let ch2_span = rga.chunks[ch2 as usize].span;
    rga.chunks[ch1 as usize].s = s2;
    rga.chunks[ch1 as usize].span += ch2_span;
    delete_chunk(rga, ch2);
    true
}

/// Try to merge tombstones around the deletion range `[start, end]`.
/// Mirrors `AbstractRga.mergeTombstones2()`.
fn merge_tombstones2<T: Clone>(rga: &mut Rga<T>, start: u32, end: u32) {
    let mut curr = Some(start);
    while let Some(ci) = curr {
        let next_ci = pos_next(&rga.chunks, ci);
        let Some(next_ci) = next_ci else {
            break;
        };
        let merged = merge_tombstones(rga, ci, next_ci);
        if !merged {
            if next_ci == end {
                if let Some(n) = pos_next(&rga.chunks, next_ci) {
                    merge_tombstones(rga, next_ci, n);
                }
                break;
            }
            curr = rga.chunks[ci as usize].s;
        }
        // If merged, curr stays the same (ci absorbed next_ci).
    }
    if let Some(left) = pos_prev(&rga.chunks, start) {
        merge_tombstones(rga, left, start);
    }
}

// ── deleteSpan ────────────────────────────────────────────────────────────

/// Delete all items in a single timestamp span.
/// Mirrors `AbstractRga.deleteSpan()`.
fn delete_span<T: Clone + ChunkData>(rga: &mut Rga<T>, tss: Tss) {
    let t1 = tss.time;
    let t2 = t1 + tss.span - 1;

    // Find the first chunk covering t1.
    let start_opt = rga.find_by_id(Ts::new(tss.sid, t1));
    let Some(start) = start_opt else {
        return;
    };

    let mut chunk_opt = Some(start);
    let mut last = start;

    while let Some(ci) = chunk_opt {
        last = ci;
        let c_id = rga.chunks[ci as usize].id;
        let c_span = rga.chunks[ci as usize].span;
        let c1 = c_id.time;
        let c2 = c1 + c_span - 1;

        if rga.chunks[ci as usize].deleted {
            if c2 >= t2 {
                break;
            }
            chunk_opt = rga.chunks[ci as usize].s;
            continue;
        }

        let delete_from_left = t1 <= c1;
        let delete_from_middle = t1 <= c2;

        if delete_from_left {
            let fully_contains = t2 >= c2;
            if fully_contains {
                // Delete the whole chunk.
                rga.chunks[ci as usize].deleted = true;
                rga.chunks[ci as usize].data = None;
                d_len(&mut rga.chunks, Some(ci), -(c_span as i64));
                if t2 <= c2 {
                    break;
                }
            } else {
                // Delete a prefix [c1, t2], keep suffix [t2+1, c2].
                let range = (t2 - c1 + 1) as usize;
                let _new_ci = split_for_delete(rga, ci, range);
                // After split: ci.span = range (the part to delete).
                let del_span = rga.chunks[ci as usize].span;
                rga.chunks[ci as usize].deleted = true;
                rga.chunks[ci as usize].data = None;
                update_len_one(&mut rga.chunks, _new_ci);
                d_len(&mut rga.chunks, Some(ci), -(del_span as i64));
                break;
            }
        } else if delete_from_middle {
            let delete_right_side = t2 >= c2;
            if delete_right_side {
                // Delete suffix [t1, c2], keep prefix [c1, t1-1].
                let offset = (t1 - c1) as usize;
                let new_ci = split_for_delete(rga, ci, offset);
                let new_span = rga.chunks[new_ci as usize].span;
                rga.chunks[new_ci as usize].deleted = true;
                rga.chunks[new_ci as usize].data = None;
                rga.chunks[new_ci as usize].len = rga.chunks[new_ci as usize]
                    .r
                    .map(|r| rga.chunks[r as usize].len)
                    .unwrap_or(0);
                d_len(&mut rga.chunks, Some(ci), -(new_span as i64));
                if t2 <= c2 {
                    break;
                }
            } else {
                // Delete middle [t1, t2], keep left [c1, t1-1] and right [t2+1, c2].
                let right = split_for_delete(rga, ci, (t2 - c1 + 1) as usize);
                let mid = split_for_delete(rga, ci, (t1 - c1) as usize);
                let mid_span = rga.chunks[mid as usize].span;
                rga.chunks[mid as usize].deleted = true;
                rga.chunks[mid as usize].data = None;
                update_len_one(&mut rga.chunks, right);
                update_len_one(&mut rga.chunks, mid);
                d_len(&mut rga.chunks, Some(ci), -(mid_span as i64));
                break;
            }
        }

        chunk_opt = rga.chunks[ci as usize].s;
    }

    merge_tombstones2(rga, start, last);
}

// ── Rga impl ──────────────────────────────────────────────────────────────

impl<T: Clone + ChunkData> Rga<T> {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            root: None,
            ids: None,
            count: 0,
        }
    }

    // ── Public accessors ──────────────────────────────────────────────────

    /// Total chunk count (including deleted tombstones).
    pub fn chunk_count(&self) -> usize {
        self.count
    }

    /// Reference to the chunk at arena index `idx`.
    pub fn slot(&self, idx: u32) -> &Chunk<T> {
        &self.chunks[idx as usize]
    }

    /// Mutable reference to the chunk at arena index `idx`.
    pub fn slot_mut(&mut self, idx: u32) -> &mut Chunk<T> {
        &mut self.chunks[idx as usize]
    }

    /// Last chunk in document order (rightmost in position tree).
    pub fn last_chunk(&self) -> Option<&Chunk<T>> {
        let mut curr = self.root;
        while let Some(idx) = curr {
            match self.chunks[idx as usize].r {
                Some(r) => curr = Some(r),
                None => return Some(&self.chunks[idx as usize]),
            }
        }
        None
    }

    // ── ID lookup ─────────────────────────────────────────────────────────

    /// Find the arena index of the chunk whose ID range contains `ts`.
    ///
    /// Uses a BST walk on the ID tree.  Mirrors `AbstractRga.findById()`.
    pub fn find_by_id(&self, ts: Ts) -> Option<u32> {
        let after_sid = ts.sid;
        let after_time = ts.time;
        let mut curr = self.ids;
        let mut chunk: Option<u32> = None;

        while let Some(ci) = curr {
            let c_id = self.chunks[ci as usize].id;
            let c_sid = c_id.sid;
            if c_sid > after_sid {
                curr = self.chunks[ci as usize].l2;
            } else if c_sid < after_sid {
                chunk = Some(ci);
                curr = self.chunks[ci as usize].r2;
            } else {
                let c_time = c_id.time;
                if c_time > after_time {
                    curr = self.chunks[ci as usize].l2;
                } else if c_time < after_time {
                    chunk = Some(ci);
                    curr = self.chunks[ci as usize].r2;
                } else {
                    chunk = Some(ci);
                    break;
                }
            }
        }

        let chunk = chunk?;
        let c = &self.chunks[chunk as usize];
        if c.id.sid != after_sid {
            return None;
        }
        if after_time < c.id.time {
            return None;
        }
        let offset = after_time - c.id.time;
        if offset >= c.span {
            return None;
        }
        Some(chunk)
    }

    // ── Insert ────────────────────────────────────────────────────────────

    /// Insert `data` (with timestamp `id`, logical span `span`) after the
    /// item identified by `after`.  If `after` is ORIGIN `(0,0)`, prepend.
    pub fn insert(&mut self, after: Ts, id: Ts, span: u64, data: T) {
        // Pre-allocate the chunk in the arena.
        let new_idx = self.chunks.len() as u32;
        self.chunks.push(Chunk::new(id, span, data));

        let is_root_insert = after.sid == 0 && after.time == 0;
        if is_root_insert {
            ins_after_root(self, after, new_idx);
            return;
        }

        // Find the chunk containing `after` via the ID tree.
        let after_chunk = {
            let after_sid = after.sid;
            let after_time = after.time;
            let mut curr = self.ids;
            let mut chunk: Option<u32> = None;
            while let Some(ci) = curr {
                let c_id = self.chunks[ci as usize].id;
                let c_sid = c_id.sid;
                if c_sid > after_sid {
                    curr = self.chunks[ci as usize].l2;
                } else if c_sid < after_sid {
                    chunk = Some(ci);
                    curr = self.chunks[ci as usize].r2;
                } else {
                    let c_time = c_id.time;
                    if c_time > after_time {
                        curr = self.chunks[ci as usize].l2;
                    } else if c_time < after_time {
                        chunk = Some(ci);
                        curr = self.chunks[ci as usize].r2;
                    } else {
                        chunk = Some(ci);
                        break;
                    }
                }
            }
            chunk.and_then(|ci| {
                let c = &self.chunks[ci as usize];
                if c.id.sid != after_sid {
                    return None;
                }
                let offset = after_time - c.id.time;
                if offset >= c.span {
                    return None;
                }
                Some((ci, offset as usize))
            })
        };

        let Some((chunk_idx, chunk_offset)) = after_chunk else {
            // Reference not found in the ID tree.  This happens when `after`
            // is the RGA node's own sentinel ID (e.g. str_id for insert-at-0),
            // which the upstream handles via `isRootInsert`.  Treat it as a
            // root insert so the chunk ends up at the correct position.
            ins_after_root(self, after, new_idx);
            return;
        };

        ins_after_chunk(self, after, chunk_idx, chunk_offset, new_idx);
    }

    // ── Append (for codec decode) ─────────────────────────────────────────

    /// Append a pre-built chunk at the document-order tail.
    ///
    /// Used by codec decoders that push chunks in their encoded (document)
    /// order.  Wires the chunk into the rightmost position of the position
    /// tree and inserts it into the ID tree.
    pub fn push_chunk(&mut self, chunk: Chunk<T>) {
        let idx = self.chunks.len() as u32;
        self.chunks.push(chunk);

        // Wire into position tree at the rightmost position.
        match self.root {
            None => {
                // First chunk ever.
                self.root = Some(idx);
                // Wire into ID tree.
                insert_id(self, idx);
            }
            Some(_) => {
                // Find the rightmost node.
                let mut rightmost = self.root.unwrap();
                while let Some(r) = self.chunks[rightmost as usize].r {
                    rightmost = r;
                }
                // Attach as right child of rightmost.
                let r_len = 0u64; // no right subtree
                let span = self.chunks[idx as usize].span;
                self.chunks[rightmost as usize].r = Some(idx);
                self.chunks[idx as usize].p = Some(rightmost);
                // idx has no children yet, so len = span (or 0 if deleted).
                let idx_len = if self.chunks[idx as usize].deleted {
                    0
                } else {
                    span
                };
                self.chunks[idx as usize].len = idx_len + r_len;
                // Propagate up.
                d_len(&mut self.chunks, Some(rightmost), idx_len as i64);
                // Wire into ID tree.
                insert_id(self, idx);
            }
        }
    }

    // ── Deletion ─────────────────────────────────────────────────────────

    /// Delete all items covered by the given timestamp spans.
    pub fn delete(&mut self, spans: &[Tss]) {
        for &tss in spans {
            delete_span(self, tss);
        }
    }

    // ── Iteration ─────────────────────────────────────────────────────────

    /// Iterator over all chunks in document order (in-order position tree).
    pub fn iter(&self) -> RgaIter<'_, T> {
        RgaIter {
            chunks: &self.chunks,
            curr: pos_first(&self.chunks, self.root),
        }
    }

    /// Iterator over live (non-deleted) chunks.
    pub fn iter_live(&self) -> impl Iterator<Item = &Chunk<T>> {
        self.iter().filter(|c| !c.deleted)
    }
}

// ── RgaIter ───────────────────────────────────────────────────────────────

pub struct RgaIter<'a, T: Clone> {
    chunks: &'a [Chunk<T>],
    curr: Option<u32>,
}

impl<'a, T: Clone> Iterator for RgaIter<'a, T> {
    type Item = &'a Chunk<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let idx = self.curr?;
        let chunk = &self.chunks[idx as usize];
        self.curr = pos_next(self.chunks, idx);
        Some(chunk)
    }
}

// ── Default ───────────────────────────────────────────────────────────────

impl<T: Clone + ChunkData> Default for Rga<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::{ts, tss};

    fn origin() -> Ts {
        ts(0, 0)
    }
    fn sid() -> u64 {
        1
    }

    #[test]
    fn insert_single_chunk() {
        let mut rga: Rga<String> = Rga::new();
        rga.insert(origin(), ts(sid(), 1), 5, "hello".to_string());
        assert_eq!(rga.chunk_count(), 1);
        assert_eq!(rga.iter().next().unwrap().data.as_deref(), Some("hello"));
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
        rga.insert(origin(), ts(1, 1), 2, "he".to_string());
        rga.insert(ts(1, 2), ts(1, 3), 3, "llo".to_string());
        // Delete 'e','l' spanning both chunks = tss(1, 2, 2)
        rga.delete(&[tss(1, 2, 2)]);
        let s: String = rga.iter_live().filter_map(|c| c.data.as_deref()).collect();
        assert_eq!(s, "hlo");
    }

    /// Convergence test: two peers apply the same concurrent inserts at the same
    /// position in different orders and must produce identical final views.
    #[test]
    fn concurrent_inserts_converge_regardless_of_application_order() {
        let build = |order: &[(u64, u64)]| -> String {
            let mut rga: Rga<String> = Rga::new();
            rga.insert(origin(), ts(1, 1), 1, "A".to_string());
            for &(sid, time) in order {
                rga.insert(ts(1, 1), ts(sid, time), 1, sid.to_string());
            }
            rga.iter_live().filter_map(|c| c.data.as_deref()).collect()
        };

        let view_a = build(&[(2, 1), (3, 1)]);
        let view_b = build(&[(3, 1), (2, 1)]);
        assert_eq!(view_a, view_b, "concurrent inserts must converge");
        let pos3 = view_a.find('3').unwrap();
        let pos2 = view_a.find('2').unwrap();
        assert!(
            pos3 < pos2,
            "higher-priority (sid=3) chunk should precede sid=2 chunk"
        );
    }

    #[test]
    fn split_at_offset_multibyte_chars() {
        let mut rga: Rga<String> = Rga::new();
        rga.insert(origin(), ts(1, 1), 3, "A😀B".to_string());
        rga.delete(&[tss(1, 2, 1)]);
        let s: String = rga.iter_live().filter_map(|c| c.data.as_deref()).collect();
        assert_eq!(s, "AB");
    }

    #[test]
    fn insert_after_mid_chunk_character_with_higher_priority() {
        let mut rga: Rga<String> = Rga::new();
        rga.insert(origin(), ts(1, 1), 5, "hello".to_string());
        rga.insert(ts(1, 3), ts(2, 1000), 1, "X".to_string());
        let s: String = rga.iter_live().filter_map(|c| c.data.as_deref()).collect();
        assert_eq!(s, "helXlo");
    }

    #[test]
    fn push_chunk_builds_sequence_in_order() {
        let mut rga: Rga<String> = Rga::new();
        rga.push_chunk(Chunk::new(ts(1, 1), 5, "hello".to_string()));
        rga.push_chunk(Chunk::new(ts(1, 6), 1, " ".to_string()));
        rga.push_chunk(Chunk::new(ts(1, 7), 5, "world".to_string()));
        let s: String = rga.iter_live().filter_map(|c| c.data.as_deref()).collect();
        assert_eq!(s, "hello world");
        assert_eq!(rga.chunk_count(), 3);
    }

    #[test]
    fn find_by_id_locates_mid_chunk_item() {
        let mut rga: Rga<String> = Rga::new();
        rga.insert(origin(), ts(1, 1), 5, "hello".to_string());
        assert!(rga.find_by_id(ts(1, 3)).is_some());
        assert!(rga.find_by_id(ts(2, 1)).is_none());
    }
}
