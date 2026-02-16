# Upstream Implementation Review (M7 Wrap-Up)

Date: 2026-02-16  
Upstream pin: `json-joy@17.67.0`  
Upstream source: `/Users/nchapman/Code/json-joy`

## Purpose

This document captures a direct side-by-side review of runtime-core Rust implementations against upstream `json-joy`, with emphasis on:

1. Module mapping clarity for maintainers.
2. Intentional compatibility quirks and why they exist.
3. Organization/documentation follow-up to avoid accidental "cleanup" regressions.

## Reviewed Module Mapping

1. Diff runtime:
- Upstream:
  - `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-diff/JsonCrdtDiff.ts`
- Rust:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/diff_runtime/mod.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/diff_runtime/object.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/diff_runtime/array.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/diff_runtime/string.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/diff_runtime/bin.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/diff_runtime/vec.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/diff_runtime/scalar.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/diff_runtime/dst_keys.rs`

2. Model/runtime:
- Upstream:
  - `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/Model.ts`
  - `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/nodes/*`
- Rust:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/model/mod.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/model_runtime/mod.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/model_runtime/{apply,decode,encode,query,rga,view}.rs`

3. Patch/clock/codecs:
- Upstream:
  - `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/Patch.ts`
  - `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/PatchBuilder.ts`
  - `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/codec/*`
  - `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/compaction.ts`
  - `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/schema.ts`
- Rust:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch/mod.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch/{decode,encode,rewrite,types}.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch_builder.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch_compact_codec.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch_compact_binary_codec.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch_verbose_codec.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch_compaction.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/schema.rs`

4. Utility diff:
- Upstream:
  - `/Users/nchapman/Code/json-joy/packages/json-joy/src/util/diff/str.ts`
  - `/Users/nchapman/Code/json-joy/packages/json-joy/src/util/diff/bin.ts`
  - `/Users/nchapman/Code/json-joy/packages/json-joy/src/util/diff/line.ts`
- Rust:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/util_diff/{str,bin,line}.rs`

5. less-db compatibility orchestration:
- Downstream reference:
  - `/Users/nchapman/Code/lessisbetter/less-platform/less-db-js/src/crdt/model-manager.ts`
- Rust:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/less_db_compat.rs`

## Compatibility Quirks (Intentional)

These are deliberate and should not be "simplified" without fixture/differential updates:

1. Patch decode permissiveness for malformed payloads:
- Rust keeps fixture-backed permissive behavior for specific malformed classes in:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch/decode.rs`
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch/types.rs`
- Rationale: upstream decoder behavior is permissive for some malformed inputs, and compatibility tests depend on that.

2. Patch binary metadata default and timeline rules:
- Rust encoder always emits CBOR `undefined` metadata default unless explicitly represented:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch/encode.rs`
- Rust canonical id checks enforce timeline/clock progression for builder-produced patches:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch_builder.rs`
- Rationale: aligns with upstream `PatchBuilder` output and fixture byte parity.

3. JSON-pack-compatible string header sizing:
- Rust intentionally uses json-pack style CBOR text header selection:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch/encode.rs`
- Rationale: canonical shortest CBOR is not always upstream output; this preserves exact bytes.

4. `ins_str` span semantics:
- Rust span uses UTF-16 code unit length for string insertion operations:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/patch/types.rs`
- Rationale: matches upstream span/accounting behavior and affects clock timeline parity.

5. Diff fallback policy for runtime-core shapes:
- Rust keeps `DiffError::UnsupportedShape` type but avoids emitting it for in-scope runtime-core JSON paths:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/diff_runtime/mod.rs`
- Rationale: compatibility policy is "produce native diff or root-replace patch", not shape rejection for covered families.

6. `Model.load(..., sid)` compatibility in less-db layer:
- Rust logical-clock load path forks session id similarly to upstream `setSid` behavior:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/less_db_compat.rs`
- Rationale: preserves expected local-session ownership semantics in downstream workflows.

7. Native no-op replay fast path in less-db compatibility layer:
- Rust uses a bounded no-op fast path (excluding `nop`, restricted envelope) before full apply:
  - `/Users/nchapman/Drive/Code/json-joy-rs/crates/json-joy-core/src/less_db_compat.rs`
- Rationale: safe optimization under fixture-backed envelope; avoids accidental clock semantics drift.

## Organization/Documentation Notes

1. Core module split is now upstream-aligned by family and should stay that way:
- `model_runtime/*`
- `diff_runtime/*`
- `patch/*`
- `model_api/*`

2. Runtime bridge policy remains:
- No production `node` subprocess calls in runtime paths.
- Oracle scripts are test tooling only under:
  - `/Users/nchapman/Drive/Code/json-joy-rs/tools/oracle-node`

3. Source-of-truth docs:
- `/Users/nchapman/Drive/Code/json-joy-rs/CORE_PARITY_MATRIX.md`
- `/Users/nchapman/Drive/Code/json-joy-rs/TASKS.md`

## Follow-up Guardrails

Before changing any quirked behavior:

1. Regenerate fixtures (`make compat-fixtures`).
2. Run upstream-mapped + differential suites first.
3. Update this review file and inline comments in touched modules with rationale.
