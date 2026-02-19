'use strict';
/**
 * Realistic lessdb-like benchmark.
 *
 * Simulates the core operations of a document database backed by JSON CRDT:
 *
 *   1. Create — initialise a document from a plain JSON object.
 *   2. Update loop — for each of UPDATE_STEPS mutations:
 *        load model binary → diff to next state → apply patch → save model binary
 *        (accumulate patches for later merge)
 *   3. Merge — take a diverged remote document and apply the local patch log.
 *
 * Two implementations are compared side-by-side:
 *
 *   upstream  — pure TypeScript json-joy (node_modules)
 *   wasm      — Rust/WASM Model (this repo) with the same high-level API shape
 *
 * Run:  node bench/lessdb-realistic.cjs
 * Env:
 *   WARMUP=20   warm-up iterations
 *   RUNS=120    measured iterations
 *   STEPS=120   update steps per scenario
 */

const {Model: TSModel} =
  require('./node_modules/json-joy/lib/json-crdt/index.js');
const {Patch: TSPatch} =
  require('./node_modules/json-joy/lib/json-crdt-patch/index.js');
const {Model: WASMModel} =
  require('../crates/json-joy-wasm/pkg/json_joy_wasm.js');

const SID      = 65536;
const WASM_SID = BigInt(SID);
const WARMUP   = Number.parseInt(process.env.WARMUP || '20', 10);
const RUNS     = Number.parseInt(process.env.RUNS   || '120', 10);
const STEPS    = Number.parseInt(process.env.STEPS  || '120', 10);

// ── Document helpers ──────────────────────────────────────────────────────────

function makeInitialDoc() {
  return {
    id:       'rec-1',
    title:    'Draft',
    body:     'hello',
    tags:     ['a', 'b'],
    counters: {views: 0, edits: 0},
    flags:    {archived: false, starred: false},
    items:    [{id: 1, done: false}, {id: 2, done: true}],
    nested:   {s: 'ab', v: [1, 2, 3]},
  };
}

function mutateDoc(prev, step) {
  const next = JSON.parse(JSON.stringify(prev));
  next.title = `Draft-${step % 17}`;
  next.body = `${next.body.slice(0, 20)}${String.fromCharCode(97 + (step % 26))}`;
  next.counters.views += 1;
  next.counters.edits += step % 3;
  next.flags.starred = step % 2 === 0;
  if (step % 5 === 0) next.flags.archived = !next.flags.archived;
  next.tags.push(`t${step % 9}`);
  if (next.tags.length > 8) next.tags.shift();
  next.items.push({id: 1000 + step, done: step % 2 === 1});
  if (next.items.length > 10) next.items.shift();
  next.nested.s = `${next.nested.s}${step % 10}`.slice(-20);
  next.nested.v[(step + 1) % next.nested.v.length] = (step * 7) % 101;
  return next;
}

function makeUpdates() {
  const docs = [];
  let cur = makeInitialDoc();
  for (let i = 0; i < STEPS; i++) {
    cur = mutateDoc(cur, i + 1);
    docs.push(cur);
  }
  return docs;
}

const updates = makeUpdates();

// ── Benchmark harness ─────────────────────────────────────────────────────────

function bench(name, fn) {
  for (let i = 0; i < WARMUP; i++) fn();
  const start = process.hrtime.bigint();
  for (let i = 0; i < RUNS; i++) fn();
  const ns = Number(process.hrtime.bigint() - start);
  return {name, ms: ns / 1e6, avg: ns / 1e6 / RUNS, ops: RUNS * 1e3 / (ns / 1e6)};
}

// ── Upstream TypeScript scenario ──────────────────────────────────────────────
//
// Mirrors `Model` from json-joy (pure TS, no WASM).

function runUpstream() {
  // Create
  const created = TSModel.create(undefined, SID);
  created.api.set(makeInitialDoc());
  created.api.flush();
  let bin = created.toBinary();

  // Update loop
  const patches = [];
  for (const next of updates) {
    const model = TSModel.fromBinary(bin);
    const patch = model.api.diff(next);
    if (patch) {
      model.applyPatch(patch);
      bin = model.toBinary();
      patches.push(patch.toBinary());
    }
  }

  // Merge: remote document diverged; apply local patches into it
  const remote = TSModel.create(undefined, SID + 1);
  remote.api.set(makeInitialDoc());
  remote.api.flush();
  const remotePatch = remote.api.diff(mutateDoc(makeInitialDoc(), 999));
  if (remotePatch) remote.applyPatch(remotePatch);
  const remoteBin = remote.toBinary();

  const merged = TSModel.fromBinary(remoteBin);
  for (const p of patches) merged.applyPatch(TSPatch.fromBinary(p));

  return {view: merged.view()};
}

// ── WASM stateless scenario ───────────────────────────────────────────────────
//
// Load model binary at the start of each update, apply diff, save back.
// Mirrors a server-side stateless handler: deserialise → update → serialise.

function runWasmStateless() {
  // Create
  const created = new WASMModel(WASM_SID);
  created.diffApply(JSON.stringify(makeInitialDoc()));
  let bin = created.toBinary();
  created.free();

  // Update loop
  const patches = [];
  for (const next of updates) {
    const model = WASMModel.fromBinary(bin);
    const patchBytes = model.diffApply(JSON.stringify(next));
    if (patchBytes.length > 0) {
      bin = model.toBinary();
      patches.push(patchBytes);
    }
    model.free();
  }

  // Merge
  const remote = new WASMModel(BigInt(SID + 1));
  remote.diffApply(JSON.stringify(makeInitialDoc()));
  remote.diffApply(JSON.stringify(mutateDoc(makeInitialDoc(), 999)));
  const remoteBin = remote.toBinary();
  remote.free();

  const merged = WASMModel.fromBinary(remoteBin);
  for (const p of patches) merged.applyPatch(p);
  const view = merged.view();
  merged.free();

  return {view};
}

// ── WASM resident scenario ────────────────────────────────────────────────────
//
// Keep one live model in memory across all updates — only export when needed.
// Best case for WASM: no deserialisation cost per update.

function runWasmResident() {
  // Create
  const model = new WASMModel(WASM_SID);
  model.diffApply(JSON.stringify(makeInitialDoc()));

  // Update loop — model stays in memory
  const patches = [];
  for (const next of updates) {
    const patchBytes = model.diffApply(JSON.stringify(next));
    if (patchBytes.length > 0) patches.push(patchBytes);
  }

  model.free();

  // Merge
  const remote = new WASMModel(BigInt(SID + 1));
  remote.diffApply(JSON.stringify(makeInitialDoc()));
  remote.diffApply(JSON.stringify(mutateDoc(makeInitialDoc(), 999)));
  const remoteBin = remote.toBinary();
  remote.free();

  const merged = WASMModel.fromBinary(remoteBin);
  for (const p of patches) merged.applyPatch(p);
  const view = merged.view();
  merged.free();

  return {view};
}

// ── Correctness check ─────────────────────────────────────────────────────────

{
  const ts      = runUpstream();
  const wasmSL  = runWasmStateless();
  const wasmRES = runWasmResident();

  const expected = JSON.stringify(ts.view);

  function check(label, got) {
    const actual = JSON.stringify(got);
    if (actual !== expected) {
      console.error(`MISMATCH  ${label}`);
      console.error(`  expected: ${expected.slice(0, 200)}`);
      console.error(`  got:      ${actual.slice(0, 200)}`);
      process.exit(1);
    }
  }

  check('wasm-stateless merged view', wasmSL.view);
  check('wasm-resident  merged view', wasmRES.view);
  console.log('correctness: OK\n');
}

// ── Benchmark ─────────────────────────────────────────────────────────────────

const results = [
  bench('upstream (ts)',        runUpstream),
  bench('wasm stateless',       runWasmStateless),
  bench('wasm resident',        runWasmResident),
];

const baseOps = results[0].ops;
console.log(`steps/scenario: ${STEPS}  warmup: ${WARMUP}  runs: ${RUNS}\n`);
for (const r of results) {
  const pct = ((r.ops / baseOps) * 100).toFixed(0);
  console.log(`${r.name.padEnd(22)} ${r.ops.toFixed(0).padStart(7)} scenarios/s  avg ${r.avg.toFixed(3)} ms  (${pct}% of upstream)`);
}
