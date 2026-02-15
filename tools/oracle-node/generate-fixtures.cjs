const fs = require('node:fs');
const path = require('node:path');
const {Model} = require('json-joy/lib/json-crdt/index.js');
const {Patch} = require('json-joy/lib/json-crdt-patch/index.js');

const ROOT = path.resolve(__dirname, '..', '..');
const OUT_DIR = path.join(ROOT, 'tests', 'compat', 'fixtures');
const UPSTREAM_VERSION = '17.67.0';

const FIXTURE_VERSION = 1;

function hex(bytes) {
  return Buffer.from(bytes).toString('hex');
}

function ensureDir(p) {
  fs.mkdirSync(p, {recursive: true});
}

function writeFixture(name, payload) {
  const file = path.join(OUT_DIR, `${name}.json`);
  fs.writeFileSync(file, JSON.stringify(payload, null, 2) + '\n', 'utf8');
}

function buildFixtureBase(name, scenario, input, expected) {
  return {
    fixture_version: FIXTURE_VERSION,
    name,
    scenario,
    input,
    expected,
    meta: {
      upstream_package: 'json-joy',
      upstream_version: UPSTREAM_VERSION,
      generator: 'tools/oracle-node/generate-fixtures.cjs',
      generated_at: new Date().toISOString()
    }
  };
}

function fixtureModelRoundTrip() {
  const sid = 70001;
  const data = {
    id: 'doc-1',
    title: 'Hello',
    body: 'World',
    done: false,
    count: 1,
    tags: ['a', 'b']
  };

  const model = Model.create(undefined, sid);
  model.api.set(data);
  model.api.flush();

  const modelBinary = model.toBinary();
  const restored = Model.fromBinary(modelBinary);

  return buildFixtureBase(
    'model_roundtrip_v1',
    'model_roundtrip',
    {sid, data},
    {
      model_binary_hex: hex(modelBinary),
      view_json: restored.view()
    }
  );
}

function fixturePatchRoundTrip() {
  const sid = 70002;
  const base = {title: 'alpha', count: 1, tags: ['a']};
  const updated = {title: 'alpha beta', count: 2, tags: ['a', 'b']};

  const model = Model.create(undefined, sid);
  model.api.set(base);
  model.api.flush();

  const patch = model.api.diff(updated);
  if (!patch) throw new Error('expected diff patch');

  const patchBinary = patch.toBinary();
  const decoded = Patch.fromBinary(patchBinary);

  return buildFixtureBase(
    'patch_roundtrip_v1',
    'patch_roundtrip',
    {sid, base, updated},
    {
      patch_binary_hex: hex(patchBinary),
      patch_span: decoded.span(),
      patch_op_count: decoded.ops.length
    }
  );
}

function fixtureForkMergeReplay() {
  const sidA = 71001;
  const sidB = 71002;
  const base = {
    title: 'Original',
    body: 'Original body',
    score: 5
  };

  const modelA = Model.create(undefined, sidA);
  modelA.api.set(base);
  modelA.api.flush();

  const modelB = modelA.fork(sidB);

  const localNext = {...base, title: 'Local Title'};
  const remoteNext = {...base, body: 'Remote body', score: 7};

  const localPatch = modelA.api.diff(localNext);
  if (!localPatch) throw new Error('expected local diff patch');
  modelA.applyPatch(localPatch);

  const remotePatch = modelB.api.diff(remoteNext);
  if (!remotePatch) throw new Error('expected remote diff patch');
  modelB.applyPatch(remotePatch);

  // Merge remote patch into local model.
  modelA.applyPatch(remotePatch);

  // Replay once more to capture idempotence behavior expectation.
  modelA.applyPatch(remotePatch);

  return buildFixtureBase(
    'fork_merge_replay_v1',
    'fork_merge_replay',
    {sid_a: sidA, sid_b: sidB, base, local_next: localNext, remote_next: remoteNext},
    {
      local_patch_hex: hex(localPatch.toBinary()),
      remote_patch_hex: hex(remotePatch.toBinary()),
      merged_view_json: modelA.view(),
      merged_model_binary_hex: hex(modelA.toBinary())
    }
  );
}

function main() {
  ensureDir(OUT_DIR);
  const fixtures = [fixtureModelRoundTrip(), fixturePatchRoundTrip(), fixtureForkMergeReplay()];

  for (const fixture of fixtures) {
    writeFixture(fixture.name, fixture);
  }

  const manifest = {
    fixture_version: FIXTURE_VERSION,
    upstream_package: 'json-joy',
    upstream_version: UPSTREAM_VERSION,
    fixtures: fixtures.map((f) => ({name: f.name, scenario: f.scenario, file: `${f.name}.json`})),
    generated_at: new Date().toISOString()
  };

  fs.writeFileSync(path.join(OUT_DIR, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n', 'utf8');

  console.log(`wrote ${fixtures.length} fixtures to ${OUT_DIR}`);
}

main();
