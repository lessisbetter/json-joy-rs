//! [`Batch`] — a sequence of patches from the same session.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/Batch.ts`.

use crate::json_crdt_patch::clock::{print_ts, Ts};
use crate::json_crdt_patch::patch::Patch;

/// A batch of patches belonging to the same session.
///
/// A batch can be rebased as a unit against a new server time.
#[derive(Debug, Clone)]
pub struct Batch {
    pub patches: Vec<Patch>,
}

impl Batch {
    pub fn new(patches: Vec<Patch>) -> Self {
        Self { patches }
    }

    /// Returns the ID of the first patch, if any.
    pub fn get_id(&self) -> Option<Ts> {
        self.patches.first().and_then(|p| p.get_id())
    }

    /// Rebases all patches in the batch starting at `server_time`.
    ///
    /// All timestamps at or after `transform_horizon` (= start of the first
    /// patch) are shifted by the same delta.
    pub fn rebase(&self, server_time: u64) -> Batch {
        let id = self.get_id().expect("BATCH_EMPTY");
        let transform_horizon = id.time;
        let mut new_patches = Vec::with_capacity(self.patches.len());
        let mut t = server_time;
        for patch in &self.patches {
            new_patches.push(patch.rebase(t, Some(transform_horizon)));
            t += patch.span();
        }
        Batch {
            patches: new_patches,
        }
    }

    /// Deep-clones the batch.
    pub fn clone_batch(&self) -> Batch {
        Batch {
            patches: self.patches.iter().map(|p| p.clone_patch()).collect(),
        }
    }
}

impl std::fmt::Display for Batch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let id_str = match self.get_id() {
            Some(id) => print_ts(id),
            None => "(nil)".to_owned(),
        };
        write!(f, "Batch {}", id_str)?;
        let len = self.patches.len();
        for (i, patch) in self.patches.iter().enumerate() {
            let is_last = i == len - 1;
            let connector = if is_last { "└─" } else { "├─" };
            write!(f, "\n{} {}", connector, patch)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::Op;

    #[test]
    fn batch_rebase() {
        let mut p1 = Patch::new();
        p1.ops.push(Op::NewObj { id: ts(1, 10) });
        let mut p2 = Patch::new();
        p2.ops.push(Op::NewArr { id: ts(1, 11) });

        let batch = Batch::new(vec![p1, p2]);
        let rebased = batch.rebase(20);
        assert_eq!(rebased.patches[0].get_id(), Some(ts(1, 20)));
        assert_eq!(rebased.patches[1].get_id(), Some(ts(1, 21)));
    }
}
