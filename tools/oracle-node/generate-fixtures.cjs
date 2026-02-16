const fs = require('node:fs');
const path = require('node:path');
const {Model} = require('json-joy/lib/json-crdt/index.js');
const {JsonCrdtDiff} = require('json-joy/lib/json-crdt-diff/JsonCrdtDiff.js');
const {Encoder: IndexedBinaryEncoder} = require('json-joy/lib/json-crdt/codec/indexed/binary/Encoder.js');
const {Decoder: IndexedBinaryDecoder} = require('json-joy/lib/json-crdt/codec/indexed/binary/Decoder.js');
const {Encoder: SidecarBinaryEncoder} = require('json-joy/lib/json-crdt/codec/sidecar/binary/Encoder.js');
const {Decoder: SidecarBinaryDecoder} = require('json-joy/lib/json-crdt/codec/sidecar/binary/Decoder.js');
const {ClockTable} = require('json-joy/lib/json-crdt-patch/codec/clock/ClockTable.js');
const {ClockEncoder} = require('json-joy/lib/json-crdt-patch/codec/clock/ClockEncoder.js');
const {ClockDecoder} = require('json-joy/lib/json-crdt-patch/codec/clock/ClockDecoder.js');
const {CrdtWriter} = require('json-joy/lib/json-crdt-patch/util/binary/CrdtWriter.js');
const {CrdtReader} = require('json-joy/lib/json-crdt-patch/util/binary/CrdtReader.js');
const {CborDecoder} = require('@jsonjoy.com/json-pack/lib/cbor/CborDecoder.js');
const patchLib = require('json-joy/lib/json-crdt-patch/index.js');
const {
  Patch,
  ts,
  tss,
  NewConOp,
  NewValOp,
  NewObjOp,
  NewVecOp,
  NewStrOp,
  NewBinOp,
  NewArrOp,
  InsValOp,
  InsObjOp,
  InsVecOp,
  InsStrOp,
  InsBinOp,
  InsArrOp,
  UpdArrOp,
  DelOp,
  NopOp,
} = patchLib;

const ROOT = path.resolve(__dirname, '..', '..');
const OUT_DIR = path.join(ROOT, 'tests', 'compat', 'fixtures');
const UPSTREAM_VERSION = '17.67.0';
const FIXTURE_VERSION = 1;
const OPCODE_BY_NAME = {
  new_con: 0,
  new_val: 1,
  new_obj: 2,
  new_vec: 3,
  new_str: 4,
  new_bin: 5,
  new_arr: 6,
  ins_val: 9,
  ins_obj: 10,
  ins_vec: 11,
  ins_str: 12,
  ins_bin: 13,
  ins_arr: 14,
  upd_arr: 15,
  del: 16,
  nop: 17,
};

function hex(bytes) {
  return Buffer.from(bytes).toString('hex');
}

function fromHex(hexStr) {
  return new Uint8Array(Buffer.from(hexStr, 'hex'));
}

function ensureDir(p) {
  fs.mkdirSync(p, {recursive: true});
}

function writeFixture(name, payload) {
  const file = path.join(OUT_DIR, `${name}.json`);
  fs.writeFileSync(file, JSON.stringify(payload, null, 2) + '\n', 'utf8');
}

function baseFixture(name, scenario, input, expected) {
  return {
    fixture_version: FIXTURE_VERSION,
    name,
    scenario,
    input,
    expected,
    meta: {
      upstream_package: 'json-joy',
      upstream_version: UPSTREAM_VERSION,
      generator: 'tools/oracle-node/generate-fixtures.cjs'
    }
  };
}

function cloneJson(x) {
  return JSON.parse(JSON.stringify(x));
}

function mulberry32(seed) {
  let t = seed >>> 0;
  return function rng() {
    t += 0x6D2B79F5;
    let r = Math.imul(t ^ (t >>> 15), 1 | t);
    r ^= r + Math.imul(r ^ (r >>> 7), 61 | r);
    return ((r ^ (r >>> 14)) >>> 0) / 4294967296;
  };
}

function randInt(rng, maxExclusive) {
  return Math.floor(rng() * maxExclusive);
}

function randString(rng, minLen, maxLen) {
  const alphabet = 'abcdefghijklmnopqrstuvwxyz0123456789';
  const len = minLen + randInt(rng, maxLen - minLen + 1);
  let out = '';
  for (let i = 0; i < len; i++) out += alphabet[randInt(rng, alphabet.length)];
  return out;
}

function randScalar(rng) {
  const t = randInt(rng, 5);
  if (t === 0) return null;
  if (t === 1) return rng() < 0.5;
  if (t === 2) return (randInt(rng, 2000) - 1000) / 10;
  return randString(rng, 0, 16);
}

function randJson(rng, depth) {
  if (depth <= 0) return randScalar(rng);
  const t = randInt(rng, 4);
  if (t <= 1) return randScalar(rng);
  if (t === 2) {
    const n = randInt(rng, 5);
    const arr = [];
    for (let i = 0; i < n; i++) arr.push(randJson(rng, depth - 1));
    return arr;
  }
  const n = randInt(rng, 5);
  const obj = {};
  for (let i = 0; i < n; i++) obj[randString(rng, 1, 10)] = randJson(rng, depth - 1);
  return obj;
}

function mkModel(data, sid) {
  const model = Model.create(undefined, sid);
  model.api.set(data);
  model.api.flush();
  return model;
}

function buildDiffFixture(name, sid, base, next) {
  const model = mkModel(base, sid);
  const patch = model.api.diff(next);

  if (!patch) {
    return baseFixture(name, 'patch_diff_apply', {sid, base, next}, {
      patch_present: false,
      base_model_binary_hex: hex(model.toBinary()),
      view_after_apply_json: model.view(),
      model_binary_after_apply_hex: hex(model.toBinary())
    });
  }

  const patchBinary = patch.toBinary();
  const decoded = Patch.fromBinary(patchBinary);
  const patchId = decoded.getId();

  model.applyPatch(patch);

  return baseFixture(name, 'patch_diff_apply', {sid, base, next}, {
    patch_present: true,
    patch_binary_hex: hex(patchBinary),
    patch_op_count: decoded.ops.length,
    patch_opcodes: decoded.ops.map((op) => OPCODE_BY_NAME[op.name()]),
    patch_span: decoded.span(),
    patch_id_sid: patchId ? patchId.sid : null,
    patch_id_time: patchId ? patchId.time : null,
    patch_next_time: decoded.nextTime(),
    view_after_apply_json: model.view(),
    model_binary_after_apply_hex: hex(model.toBinary())
  });
}

function tsFromTuple(t) {
  return ts(t[0], t[1]);
}

function canonicalPatchFromModel(input) {
  const patch = new Patch();
  patch.meta = undefined;
  for (const op of input.ops) {
    switch (op.op) {
      case 'new_con':
        patch.ops.push(new NewConOp(tsFromTuple(op.id), op.value));
        break;
      case 'new_val':
        patch.ops.push(new NewValOp(tsFromTuple(op.id)));
        break;
      case 'new_obj':
        patch.ops.push(new NewObjOp(tsFromTuple(op.id)));
        break;
      case 'new_str':
        patch.ops.push(new NewStrOp(tsFromTuple(op.id)));
        break;
      case 'new_arr':
        patch.ops.push(new NewArrOp(tsFromTuple(op.id)));
        break;
      case 'ins_val':
        patch.ops.push(new InsValOp(tsFromTuple(op.id), tsFromTuple(op.obj), tsFromTuple(op.val)));
        break;
      case 'ins_obj':
        patch.ops.push(
          new InsObjOp(
            tsFromTuple(op.id),
            tsFromTuple(op.obj),
            op.data.map(([k, v]) => [k, tsFromTuple(v)])
          )
        );
        break;
      case 'ins_str':
        patch.ops.push(new InsStrOp(tsFromTuple(op.id), tsFromTuple(op.obj), tsFromTuple(op.ref), op.data));
        break;
      case 'ins_arr':
        patch.ops.push(
          new InsArrOp(
            tsFromTuple(op.id),
            tsFromTuple(op.obj),
            tsFromTuple(op.ref),
            op.data.map((v) => tsFromTuple(v))
          )
        );
        break;
      case 'nop':
        patch.ops.push(new NopOp(tsFromTuple(op.id), op.len));
        break;
      default:
        throw new Error(`unsupported canonical op: ${op.op}`);
    }
  }
  return patch;
}

function buildCanonicalEncodeFixture(name, input) {
  const patch = canonicalPatchFromModel(input);
  const binary = patch.toBinary();
  return baseFixture(name, 'patch_canonical_encode', input, {
    patch_binary_hex: hex(binary),
    patch_op_count: patch.ops.length,
    patch_span: patch.span(),
    patch_opcodes: input.ops.map((op) => OPCODE_BY_NAME[op.op]),
  });
}

function allCanonicalEncodeFixtures() {
  const sid = 74001;
  const fixtures = [
    buildCanonicalEncodeFixture('patch_canonical_root_scalar_v1', {
      sid,
      time: 1,
      meta_kind: 'undefined',
      ops: [
        {op: 'new_con', id: [sid, 1], value: 7},
        {op: 'ins_val', id: [sid, 2], obj: [0, 0], val: [sid, 1]},
      ],
    }),
    buildCanonicalEncodeFixture('patch_canonical_object_map_v1', {
      sid,
      time: 1,
      meta_kind: 'undefined',
      ops: [
        {op: 'new_obj', id: [sid, 1]},
        {op: 'ins_val', id: [sid, 2], obj: [0, 0], val: [sid, 1]},
        {op: 'new_con', id: [sid, 3], value: 1},
        {op: 'new_con', id: [sid, 4], value: 'x'},
        {
          op: 'ins_obj',
          id: [sid, 5],
          obj: [sid, 1],
          data: [
            ['a', [sid, 3]],
            ['b', [sid, 4]],
          ],
        },
      ],
    }),
    buildCanonicalEncodeFixture('patch_canonical_array_insert_v1', {
      sid,
      time: 1,
      meta_kind: 'undefined',
      ops: [
        {op: 'new_arr', id: [sid, 1]},
        {op: 'ins_val', id: [sid, 2], obj: [0, 0], val: [sid, 1]},
        {op: 'new_con', id: [sid, 3], value: 10},
        {op: 'new_con', id: [sid, 4], value: 20},
        {op: 'ins_arr', id: [sid, 5], obj: [sid, 1], ref: [sid, 1], data: [[sid, 3], [sid, 4]]},
      ],
    }),
    buildCanonicalEncodeFixture('patch_canonical_string_with_nop_v1', {
      sid,
      time: 1,
      meta_kind: 'undefined',
      ops: [
        {op: 'new_str', id: [sid, 1]},
        {op: 'ins_val', id: [sid, 2], obj: [0, 0], val: [sid, 1]},
        {op: 'ins_str', id: [sid, 3], obj: [sid, 1], ref: [sid, 1], data: 'hello'},
        {op: 'nop', id: [sid, 8], len: 2},
      ],
    }),
  ];

  const rng = mulberry32(0x70617463);
  for (let i = 0; i < 16; i++) {
    const baseTime = 1 + (i * 8);
    const v1 = randScalar(rng);
    const v2 = randScalar(rng);
    fixtures.push(
      buildCanonicalEncodeFixture(`patch_canonical_generated_${String(i + 1).padStart(2, '0')}_v1`, {
        sid,
        time: baseTime,
        meta_kind: 'undefined',
        ops: [
          {op: 'new_obj', id: [sid, baseTime]},
          {op: 'ins_val', id: [sid, baseTime + 1], obj: [0, 0], val: [sid, baseTime]},
          {op: 'new_con', id: [sid, baseTime + 2], value: v1},
          {op: 'new_con', id: [sid, baseTime + 3], value: v2},
          {
            op: 'ins_obj',
            id: [sid, baseTime + 4],
            obj: [sid, baseTime],
            data: [
              [`k${i}`, [sid, baseTime + 2]],
              [`m${i}`, [sid, baseTime + 3]],
            ],
          },
          {op: 'nop', id: [sid, baseTime + 5], len: 1},
        ],
      }),
    );
  }

  return fixtures;
}

function buildDecodeErrorFixture(name, binaryHex) {
  let message = 'UNKNOWN';
  try {
    Patch.fromBinary(fromHex(binaryHex));
    message = 'NO_ERROR';
  } catch (err) {
    message = err instanceof Error ? err.message : String(err);
  }

  return baseFixture(name, 'patch_decode_error', {patch_binary_hex: binaryHex}, {
    error_message: message
  });
}

function allDiffFixtures() {
  const fixtures = [];

  const cases = [
    // Scalars and root replacement
    {name: 'diff_root_number_to_number_v1', sid: 72001, base: 1, next: 2},
    {name: 'diff_root_string_to_string_v1', sid: 72002, base: 'a', next: 'ab'},
    {name: 'diff_root_bool_to_bool_v1', sid: 72003, base: true, next: false},
    {name: 'diff_root_obj_to_scalar_v1', sid: 72004, base: {a: 1}, next: 5},
    {name: 'diff_root_scalar_to_obj_v1', sid: 72005, base: 5, next: {a: 1}},

    // No-op
    {name: 'diff_noop_object_v1', sid: 72006, base: {a: 1, b: 'x'}, next: {a: 1, b: 'x'}},
    {name: 'diff_noop_array_v1', sid: 72007, base: [1, 2, 3], next: [1, 2, 3]},

    // Strings
    {name: 'diff_string_append_v1', sid: 72008, base: {txt: 'abc'}, next: {txt: 'abcd'}},
    {name: 'diff_string_insert_mid_v1', sid: 72009, base: {txt: 'ac'}, next: {txt: 'abc'}},
    {name: 'diff_string_delete_mid_v1', sid: 72010, base: {txt: 'abcde'}, next: {txt: 'abde'}},
    {name: 'diff_string_replace_v1', sid: 72011, base: {txt: 'kitten'}, next: {txt: 'sitting'}},

    // Objects
    {name: 'diff_object_add_key_v1', sid: 72012, base: {a: 1}, next: {a: 1, b: 2}},
    {name: 'diff_object_remove_key_v1', sid: 72013, base: {a: 1, b: 2}, next: {a: 1}},
    {name: 'diff_object_update_key_v1', sid: 72014, base: {a: 1, b: 2}, next: {a: 9, b: 2}},
    {name: 'diff_object_multi_change_v1', sid: 72015, base: {a: 1, b: 2}, next: {a: 9, c: 3}},

    // Arrays
    {name: 'diff_array_append_v1', sid: 72016, base: {arr: [1, 2]}, next: {arr: [1, 2, 3]}},
    {name: 'diff_array_insert_mid_v1', sid: 72017, base: {arr: [1, 3]}, next: {arr: [1, 2, 3]}},
    {name: 'diff_array_delete_mid_v1', sid: 72018, base: {arr: [1, 2, 3]}, next: {arr: [1, 3]}},
    {name: 'diff_array_replace_elem_v1', sid: 72019, base: {arr: [1, 2, 3]}, next: {arr: [1, 9, 3]}},
    {name: 'diff_array_clear_v1', sid: 72020, base: {arr: [1, 2, 3]}, next: {arr: []}},

    // Nested
    {name: 'diff_nested_object_v1', sid: 72021, base: {a: {b: {c: 1}}}, next: {a: {b: {c: 2}}}},
    {name: 'diff_nested_array_v1', sid: 72022, base: {a: [{x: 1}, {x: 2}]}, next: {a: [{x: 1}, {x: 3}]}},
    {name: 'diff_nested_combo_v1', sid: 72023, base: {a: {txt: 'ab', arr: [1, 2]}}, next: {a: {txt: 'abc', arr: [2, 1]}}},

    // Mixed realistic docs
    {
      name: 'diff_doc_update_title_v1',
      sid: 72024,
      base: {id: 'd1', title: 'Old', body: 'Body', score: 1, tags: ['a']},
      next: {id: 'd1', title: 'New', body: 'Body', score: 1, tags: ['a']}
    },
    {
      name: 'diff_doc_update_body_and_score_v1',
      sid: 72025,
      base: {id: 'd1', title: 'T', body: 'Body', score: 1, tags: ['a']},
      next: {id: 'd1', title: 'T', body: 'Body 2', score: 2, tags: ['a']}
    },
    {
      name: 'diff_doc_tag_add_v1',
      sid: 72026,
      base: {id: 'd1', tags: ['a']},
      next: {id: 'd1', tags: ['a', 'b']}
    },
    {
      name: 'diff_doc_tag_remove_v1',
      sid: 72027,
      base: {id: 'd1', tags: ['a', 'b']},
      next: {id: 'd1', tags: ['b']}
    },

    // Larger strings
    {
      name: 'diff_long_string_insert_v1',
      sid: 72028,
      base: {txt: 'The quick brown fox jumps over the lazy dog'},
      next: {txt: 'The quick agile brown fox jumps over the lazy dog'}
    },
    {
      name: 'diff_long_string_delete_v1',
      sid: 72029,
      base: {txt: 'The quick brown fox jumps over the lazy dog'},
      next: {txt: 'The quick fox jumps over the lazy dog'}
    },

    // Array of objects
    {
      name: 'diff_array_object_insert_v1',
      sid: 72030,
      base: {items: [{id: 1}, {id: 3}]},
      next: {items: [{id: 1}, {id: 2}, {id: 3}]}
    },
    {
      name: 'diff_array_object_update_v1',
      sid: 72031,
      base: {items: [{id: 1, n: 'a'}, {id: 2, n: 'b'}]},
      next: {items: [{id: 1, n: 'a'}, {id: 2, n: 'B'}]}
    },

    // Edge-ish small docs
    {name: 'diff_empty_obj_to_key_v1', sid: 72032, base: {}, next: {a: 1}},
    {name: 'diff_key_to_empty_obj_v1', sid: 72033, base: {a: 1}, next: {}},
    {name: 'diff_empty_arr_to_values_v1', sid: 72034, base: [], next: [1, 2, 3]},
    {name: 'diff_values_to_empty_arr_v1', sid: 72035, base: [1, 2, 3], next: []},

    // Null handling
    {name: 'diff_null_to_object_v1', sid: 72036, base: null, next: {a: 1}},
    {name: 'diff_object_to_null_v1', sid: 72037, base: {a: 1}, next: null},

    // Deep path edits
    {name: 'diff_deep_path_edit_v1', sid: 72038, base: {a: {b: {c: {d: 'x'}}}}, next: {a: {b: {c: {d: 'y'}}}}},
    {name: 'diff_deep_path_add_v1', sid: 72039, base: {a: {b: {}}}, next: {a: {b: {c: 1}}}},
    {name: 'diff_deep_path_remove_v1', sid: 72040, base: {a: {b: {c: 1}}}, next: {a: {b: {}}}}
  ];

  for (const c of cases) {
    fixtures.push(buildDiffFixture(c.name, c.sid, cloneJson(c.base), cloneJson(c.next)));
  }

  return fixtures;
}

function allDecodeErrorFixtures() {
  const invalid = [
    {name: 'decode_error_empty_v1', hex: ''},
    {name: 'decode_error_one_byte_v1', hex: '00'},
    {name: 'decode_error_two_bytes_v1', hex: '0000'},
    {name: 'decode_error_random_4_v1', hex: 'deadbeef'},
    {name: 'decode_error_random_8_v1', hex: '0123456789abcdef'},
    {name: 'decode_error_ff_16_v1', hex: 'ffffffffffffffffffffffffffffffff'},
    {name: 'decode_error_ascii_json_v1', hex: Buffer.from('{"x":1}', 'utf8').toString('hex')},
    {name: 'decode_error_sparse_v1', hex: '0100000000000000'},
    {name: 'decode_error_short_header_v1', hex: '0102'},
    {name: 'decode_error_long_random_v1', hex: 'abcd'.repeat(32)},
  ];

  const rng = mulberry32(0xdec0de);
  for (let i = 0; i < 15; i++) {
    const len = 3 + randInt(rng, 28);
    const bytes = [];
    for (let j = 0; j < len; j++) bytes.push(randInt(rng, 256));
    invalid.push({
      name: `decode_error_random_extra_${String(i + 1).padStart(2, '0')}_v1`,
      hex: Buffer.from(bytes).toString('hex'),
    });
  }

  return invalid.map((v) => buildDecodeErrorFixture(v.name, v.hex));
}

function buildModelFixture(name, sid, data) {
  const model = sid === 1 ? Model.withServerClock(undefined, 1) : Model.create(undefined, sid);
  model.api.set(cloneJson(data));
  model.api.flush();
  return buildModelFixtureFromModel(name, model, {sid, data});
}

function buildModelFixtureFromModel(name, model, input) {
  const binary = model.toBinary();
  const restored = Model.fromBinary(binary);

  return baseFixture(name, 'model_roundtrip', input, {
    model_binary_hex: hex(binary),
    view_json: restored.view()
  });
}

function allModelFixtures() {
  const cases = [
    {name: 'model_roundtrip_scalar_number_v1', sid: 73001, data: 1},
    {name: 'model_roundtrip_scalar_string_v1', sid: 73002, data: 'hello'},
    {name: 'model_roundtrip_scalar_bool_v1', sid: 73003, data: true},
    {name: 'model_roundtrip_scalar_null_v1', sid: 73004, data: null},
    {name: 'model_roundtrip_object_simple_v1', sid: 73005, data: {a: 1, b: 'x'}},
    {name: 'model_roundtrip_object_nested_v1', sid: 73006, data: {a: {b: {c: 1}}}},
    {name: 'model_roundtrip_object_many_keys_v1', sid: 73007, data: {a: 1, b: 2, c: 3, d: 4}},
    {name: 'model_roundtrip_array_numbers_v1', sid: 73008, data: [1, 2, 3]},
    {name: 'model_roundtrip_array_objects_v1', sid: 73009, data: [{id: 1}, {id: 2}]},
    {name: 'model_roundtrip_array_nested_v1', sid: 73010, data: [[1], [2, 3], []]},
    {name: 'model_roundtrip_mixed_doc_v1', sid: 73011, data: {id: 'd1', title: 'T', tags: ['a'], done: false}},
    {name: 'model_roundtrip_empty_object_v1', sid: 73012, data: {}},
    {name: 'model_roundtrip_empty_array_v1', sid: 73013, data: []},
    {name: 'model_roundtrip_long_string_v1', sid: 73014, data: {txt: 'The quick brown fox jumps over the lazy dog'}},
    {name: 'model_roundtrip_deep_v1', sid: 73015, data: {a: {b: {c: {d: {e: 1}}}}}},
    {name: 'model_roundtrip_unicode_v1', sid: 73016, data: {txt: 'hello-π-✓'}},
    {name: 'model_roundtrip_numbers_v1', sid: 73017, data: {n1: 0, n2: -1, n3: 123.456}},
    {name: 'model_roundtrip_boolean_map_v1', sid: 73018, data: {a: true, b: false}},
    {name: 'model_roundtrip_nullable_fields_v1', sid: 73019, data: {a: null, b: 1, c: null}},
    {name: 'model_roundtrip_complex_v1', sid: 73020, data: {meta: {v: 1}, items: [{k: 'a'}, {k: 'b'}], active: true}},
    {name: 'model_roundtrip_server_scalar_number_v1', sid: 1, data: 7},
    {name: 'model_roundtrip_server_object_v1', sid: 1, data: {a: 1, b: 'srv'}},
    {name: 'model_roundtrip_server_array_v1', sid: 1, data: [1, 2, 3]},
    {name: 'model_roundtrip_server_nested_v1', sid: 1, data: {doc: {title: 'server', flags: [true, false]}}},
  ];

  const rng = mulberry32(0x5eedC0de);
  for (let i = 0; i < 40; i++) {
    cases.push({
      name: `model_roundtrip_random_${String(i + 1).padStart(2, '0')}_v1`,
      sid: 73100 + i,
      data: randJson(rng, 4),
    });
  }

  const fixtures = cases.map((c) => buildModelFixture(c.name, c.sid, c.data));

  {
    const sid = 73201;
    const model = Model.create(undefined, sid);
    const patch = new Patch();
    patch.ops.push(new NewVecOp(ts(sid, 1)));
    patch.ops.push(new InsValOp(ts(sid, 2), ts(0, 0), ts(sid, 1)));
    patch.ops.push(new NewConOp(ts(sid, 3), 7));
    patch.ops.push(new NewConOp(ts(sid, 4), 'x'));
    patch.ops.push(new InsVecOp(ts(sid, 5), ts(sid, 1), [[0, ts(sid, 3)], [2, ts(sid, 4)]]));
    model.applyPatch(patch);
    fixtures.push(
      buildModelFixtureFromModel('model_roundtrip_vec_sparse_v1', model, {
        sid,
        recipe: 'patch_apply',
        ops: ['new_vec', 'ins_val', 'new_con', 'new_con', 'ins_vec'],
      }),
    );
  }

  {
    const sid = 73202;
    const model = Model.create(undefined, sid);
    const patch = new Patch();
    patch.ops.push(new NewBinOp(ts(sid, 1)));
    patch.ops.push(new InsValOp(ts(sid, 2), ts(0, 0), ts(sid, 1)));
    patch.ops.push(new InsBinOp(ts(sid, 3), ts(sid, 1), ts(sid, 1), new Uint8Array([1, 2, 3, 4])));
    patch.ops.push(new DelOp(ts(sid, 7), ts(sid, 1), [tss(sid, 4, 1)]));
    model.applyPatch(patch);
    fixtures.push(
      buildModelFixtureFromModel('model_roundtrip_bin_tombstone_v1', model, {
        sid,
        recipe: 'patch_apply',
        ops: ['new_bin', 'ins_val', 'ins_bin', 'del'],
      }),
    );
  }

  {
    const sid = 73203;
    const model = Model.create(undefined, sid);
    const patch = new Patch();
    patch.ops.push(new NewStrOp(ts(sid, 1)));
    patch.ops.push(new InsValOp(ts(sid, 2), ts(0, 0), ts(sid, 1)));
    patch.ops.push(new InsStrOp(ts(sid, 3), ts(sid, 1), ts(sid, 1), 'abcd'));
    patch.ops.push(new DelOp(ts(sid, 7), ts(sid, 1), [tss(sid, 4, 1)]));
    model.applyPatch(patch);
    fixtures.push(
      buildModelFixtureFromModel('model_roundtrip_str_tombstone_v1', model, {
        sid,
        recipe: 'patch_apply',
        ops: ['new_str', 'ins_val', 'ins_str', 'del'],
      }),
    );
  }

  {
    const sid = 73204;
    const model = Model.create(undefined, sid);
    const patch = new Patch();
    patch.ops.push(new NewArrOp(ts(sid, 1)));
    patch.ops.push(new InsValOp(ts(sid, 2), ts(0, 0), ts(sid, 1)));
    patch.ops.push(new NewConOp(ts(sid, 3), 1));
    patch.ops.push(new NewConOp(ts(sid, 4), 2));
    patch.ops.push(new NewConOp(ts(sid, 5), 3));
    patch.ops.push(new InsArrOp(ts(sid, 6), ts(sid, 1), ts(sid, 1), [ts(sid, 3), ts(sid, 4), ts(sid, 5)]));
    patch.ops.push(new DelOp(ts(sid, 9), ts(sid, 1), [tss(sid, 7, 1)]));
    model.applyPatch(patch);
    fixtures.push(
      buildModelFixtureFromModel('model_roundtrip_arr_tombstone_v1', model, {
        sid,
        recipe: 'patch_apply',
        ops: ['new_arr', 'ins_val', 'new_con', 'new_con', 'new_con', 'ins_arr', 'del'],
      }),
    );
  }

  return fixtures;
}

function buildModelDecodeErrorFixture(name, modelBinaryHex) {
  let message = 'UNKNOWN';
  try {
    Model.fromBinary(fromHex(modelBinaryHex));
    message = 'NO_ERROR';
  } catch (err) {
    message = err instanceof Error ? err.message : String(err);
  }

  return baseFixture(name, 'model_decode_error', {model_binary_hex: modelBinaryHex}, {
    error_message: message
  });
}

function allModelDecodeErrorFixtures() {
  const invalid = [
    {name: 'model_decode_error_empty_v1', hex: ''},
    {name: 'model_decode_error_one_byte_v1', hex: '00'},
    {name: 'model_decode_error_two_bytes_v1', hex: '0000'},
    {name: 'model_decode_error_random_4_v1', hex: 'deadbeef'},
    {name: 'model_decode_error_random_8_v1', hex: '0123456789abcdef'},
    {name: 'model_decode_error_ff_16_v1', hex: 'ffffffffffffffffffffffffffffffff'},
    {name: 'model_decode_error_ascii_json_v1', hex: Buffer.from('{\"x\":1}', 'utf8').toString('hex')},
    {name: 'model_decode_error_long_random_v1', hex: 'abcd'.repeat(32)},
    {name: 'model_decode_error_clock_offset_overflow_v1', hex: 'ffffffff000000'},
    {name: 'model_decode_error_clock_offset_truncated_v1', hex: '00000010ffff'},
    {name: 'model_decode_error_clock_table_len_zero_v1', hex: '000000008000'},
    {name: 'model_decode_error_clock_table_short_tuple_v1', hex: '000000000181'},
    {name: 'model_decode_error_clock_table_bad_varint_v1', hex: '0000000001808080808080808080'},
    {name: 'model_decode_error_trunc_con_v1', hex: '0000000000001000'},
    {name: 'model_decode_error_trunc_obj_v1', hex: '000000000000204100'},
    {name: 'model_decode_error_trunc_vec_v1', hex: '0000000000003001'},
    {name: 'model_decode_error_trunc_str_v1', hex: '000000000000400110'},
    {name: 'model_decode_error_trunc_bin_v1', hex: '000000000000500110'},
    {name: 'model_decode_error_trunc_arr_v1', hex: '000000000000600110'},
    {name: 'model_decode_error_server_bad_preamble_v1', hex: '80'},
    {name: 'model_decode_error_server_trunc_time_v1', hex: '8080'},
    {name: 'model_decode_error_mixed_server_logical_v1', hex: '800100000000'},
  ];

  return invalid.map((v) => buildModelDecodeErrorFixture(v.name, v.hex));
}

function utf8Bytes(s) {
  return Buffer.from(s, 'utf8');
}

function writeU32be(out, n) {
  out.push((n >>> 24) & 0xff, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff);
}

function writeVu57(out, n) {
  let value = n >>> 0;
  let hi = Math.floor(n / 0x100000000);
  for (let i = 0; i < 7; i++) {
    const b = value & 0x7f;
    value = (value >>> 7) | ((hi & 0x7f) << 25);
    hi = hi >>> 7;
    if (value === 0 && hi === 0) {
      out.push(b);
      return;
    }
    out.push(b | 0x80);
  }
  out.push(value & 0xff);
}

function writeB1vu56(out, flag, n) {
  let value = n;
  const low6 = value & 0x3f;
  value = Math.floor(value / 64);
  let first = ((flag & 1) << 7) | low6;
  if (value === 0) {
    out.push(first);
    return;
  }
  first |= 0x40;
  out.push(first);
  for (let i = 0; i < 6; i++) {
    const b = value & 0x7f;
    value = Math.floor(value / 128);
    if (value === 0) {
      out.push(b);
      return;
    }
    out.push(b | 0x80);
  }
  out.push(value & 0xff);
}

function writeCbor(out, v) {
  if (v === null) {
    out.push(0xf6);
    return;
  }
  if (v === undefined) {
    out.push(0xf7);
    return;
  }
  if (typeof v === 'boolean') {
    out.push(v ? 0xf5 : 0xf4);
    return;
  }
  if (typeof v === 'number') {
    if (!Number.isInteger(v)) throw new Error('canonical encoder supports integer numbers only');
    if (v >= 0) writeCborMajor(out, 0, v);
    else writeCborMajor(out, 1, -1 - v);
    return;
  }
  if (typeof v === 'string') {
    const b = utf8Bytes(v);
    writeCborMajor(out, 3, b.length);
    for (const x of b) out.push(x);
    return;
  }
  throw new Error(`unsupported cbor value: ${v}`);
}

function writeCborMajor(out, major, n) {
  if (n < 24) {
    out.push((major << 5) | n);
  } else if (n < 256) {
    out.push((major << 5) | 24, n);
  } else if (n < 65536) {
    out.push((major << 5) | 25, (n >> 8) & 0xff, n & 0xff);
  } else {
    out.push((major << 5) | 26, (n >> 24) & 0xff, (n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff);
  }
}

function makeLogicalIdWriter(clockTable) {
  const indexBySid = new Map();
  const baseBySid = new Map();
  for (let i = 0; i < clockTable.length; i++) {
    indexBySid.set(clockTable[i][0], i);
    baseBySid.set(clockTable[i][0], clockTable[i][1]);
  }
  return (out, id) => {
    const sid = id[0];
    const time = id[1];
    const idx = indexBySid.get(sid);
    if (idx === undefined) throw new Error(`sid ${sid} missing from clock_table`);
    const base = baseBySid.get(sid);
    const diff = time - base;
    if (idx <= 7 && diff >= 0 && diff <= 15) out.push((idx << 4) | diff);
    else {
      writeB1vu56(out, 0, idx);
      writeVu57(out, diff);
    }
  };
}

function encodeModelCanonical(input) {
  const mode = input.mode;
  const root = input.root;
  const rootBytes = [];
  const encodeId =
    mode === 'server'
      ? (out, id) => writeVu57(out, id[1])
      : makeLogicalIdWriter(input.clock_table);

  const writeNode = (out, node) => {
    encodeId(out, node.id);
    const kind = node.kind;
    switch (kind) {
      case 'con': {
        out.push(0b00000000);
        writeCbor(out, node.value);
        break;
      }
      case 'val': {
        out.push(0b00100000);
        writeNode(out, node.child);
        break;
      }
      case 'obj': {
        const entries = node.entries || [];
        writeTypeLen(out, 2, entries.length);
        for (const e of entries) {
          writeCbor(out, e.key);
          writeNode(out, e.value);
        }
        break;
      }
      case 'vec': {
        const values = node.values || [];
        writeTypeLen(out, 3, values.length);
        for (const v of values) {
          if (v === null) out.push(0);
          else writeNode(out, v);
        }
        break;
      }
      case 'str': {
        const chunks = node.chunks || [];
        writeTypeLen(out, 4, chunks.length);
        for (const ch of chunks) {
          encodeId(out, ch.id);
          if (Object.prototype.hasOwnProperty.call(ch, 'text')) writeCbor(out, ch.text);
          else writeCbor(out, ch.deleted);
        }
        break;
      }
      case 'bin': {
        const chunks = node.chunks || [];
        writeTypeLen(out, 5, chunks.length);
        for (const ch of chunks) {
          encodeId(out, ch.id);
          if (Object.prototype.hasOwnProperty.call(ch, 'deleted')) {
            writeB1vu56(out, 1, ch.deleted);
          } else {
            const b = fromHex(ch.bytes_hex);
            writeB1vu56(out, 0, b.length);
            for (const x of b) out.push(x);
          }
        }
        break;
      }
      case 'arr': {
        const chunks = node.chunks || [];
        writeTypeLen(out, 6, chunks.length);
        for (const ch of chunks) {
          encodeId(out, ch.id);
          if (Object.prototype.hasOwnProperty.call(ch, 'deleted')) {
            writeB1vu56(out, 1, ch.deleted);
          } else {
            const vals = ch.values || [];
            writeB1vu56(out, 0, vals.length);
            for (const v of vals) writeNode(out, v);
          }
        }
        break;
      }
      default:
        throw new Error(`unsupported canonical model kind: ${kind}`);
    }
  };

  const writeTypeLen = (out, major, len) => {
    if (len < 31) out.push((major << 5) | len);
    else {
      out.push((major << 5) | 31);
      writeVu57(out, len);
    }
  };

  writeNode(rootBytes, root);

  if (mode === 'server') {
    const out = [];
    out.push(0x80);
    writeVu57(out, input.server_time);
    out.push(...rootBytes);
    return new Uint8Array(out);
  }

  const out = [];
  writeU32be(out, rootBytes.length);
  out.push(...rootBytes);
  const clockTable = input.clock_table;
  writeVu57(out, clockTable.length);
  for (const t of clockTable) {
    writeVu57(out, t[0]);
    writeVu57(out, t[1]);
  }
  return new Uint8Array(out);
}

function buildModelCanonicalEncodeFixture(name, input) {
  const binary = encodeModelCanonical(input);
  const restored = Model.fromBinary(binary);
  const view = restored.view();
  return baseFixture(name, 'model_canonical_encode', input, {
    model_binary_hex: hex(binary),
    view_json: view === undefined ? null : view,
    decode_error_message: 'NO_ERROR',
  });
}

function allModelCanonicalEncodeFixtures() {
  const sid = 74111;
  const clockTable = [[sid, 1]];
  return [
    buildModelCanonicalEncodeFixture('model_canonical_encode_logical_scalar_v1', {
      mode: 'logical',
      clock_table: clockTable,
      root: {id: [sid, 1], kind: 'con', value: 7},
    }),
    buildModelCanonicalEncodeFixture('model_canonical_encode_logical_object_v1', {
      mode: 'logical',
      clock_table: clockTable,
      root: {
        id: [sid, 1],
        kind: 'obj',
        entries: [
          {key: 'a', value: {id: [sid, 2], kind: 'con', value: 1}},
          {key: 'b', value: {id: [sid, 3], kind: 'con', value: 'x'}},
        ],
      },
    }),
    buildModelCanonicalEncodeFixture('model_canonical_encode_logical_vec_v1', {
      mode: 'logical',
      clock_table: clockTable,
      root: {
        id: [sid, 1],
        kind: 'vec',
        values: [
          {id: [sid, 2], kind: 'con', value: 7},
          null,
          {id: [sid, 3], kind: 'con', value: 'x'},
        ],
      },
    }),
    buildModelCanonicalEncodeFixture('model_canonical_encode_logical_bin_v1', {
      mode: 'logical',
      clock_table: clockTable,
      root: {
        id: [sid, 1],
        kind: 'bin',
        chunks: [
          {id: [sid, 2], bytes_hex: '01020304'},
          {id: [sid, 6], deleted: 1},
        ],
      },
    }),
    buildModelCanonicalEncodeFixture('model_canonical_encode_server_scalar_v1', {
      mode: 'server',
      server_time: 5,
      root: {id: [1, 5], kind: 'con', value: 'srv'},
    }),
    buildModelCanonicalEncodeFixture('model_canonical_encode_server_arr_v1', {
      mode: 'server',
      server_time: 9,
      root: {
        id: [1, 6],
        kind: 'arr',
        chunks: [
          {
            id: [1, 7],
            values: [
              {id: [1, 8], kind: 'con', value: 1},
              {id: [1, 9], kind: 'con', value: 2},
            ],
          },
        ],
      },
    }),
    buildModelCanonicalEncodeFixture('model_canonical_encode_logical_arr_nested_v1', {
      mode: 'logical',
      clock_table: clockTable,
      root: {
        id: [sid, 1],
        kind: 'arr',
        chunks: [
          {
            id: [sid, 2],
            values: [
              {id: [sid, 3], kind: 'con', value: 1},
              {
                id: [sid, 4],
                kind: 'obj',
                entries: [{key: 'k', value: {id: [sid, 5], kind: 'con', value: 'v'}}],
              },
            ],
          },
        ],
      },
    }),
    buildModelCanonicalEncodeFixture('model_canonical_encode_logical_str_tombstone_v1', {
      mode: 'logical',
      clock_table: clockTable,
      root: {
        id: [sid, 1],
        kind: 'str',
        chunks: [
          {id: [sid, 2], text: 'abcd'},
          {id: [sid, 6], deleted: 1},
          {id: [sid, 7], text: 'Z'},
        ],
      },
    }),
    buildModelCanonicalEncodeFixture('model_canonical_encode_logical_bin_multi_chunk_v1', {
      mode: 'logical',
      clock_table: clockTable,
      root: {
        id: [sid, 1],
        kind: 'bin',
        chunks: [
          {id: [sid, 2], bytes_hex: '0102'},
          {id: [sid, 4], deleted: 1},
          {id: [sid, 5], bytes_hex: '0a0b0c'},
        ],
      },
    }),
    buildModelCanonicalEncodeFixture('model_canonical_encode_logical_obj_deep_v1', {
      mode: 'logical',
      clock_table: clockTable,
      root: {
        id: [sid, 1],
        kind: 'obj',
        entries: [
          {
            key: 'doc',
            value: {
              id: [sid, 2],
              kind: 'obj',
              entries: [
                {key: 'title', value: {id: [sid, 3], kind: 'con', value: 'Hello'}},
                {
                  key: 'tags',
                  value: {
                    id: [sid, 4],
                    kind: 'arr',
                    chunks: [{id: [sid, 5], values: [{id: [sid, 6], kind: 'con', value: 'x'}]}],
                  },
                },
              ],
            },
          },
        ],
      },
    }),
    buildModelCanonicalEncodeFixture('model_canonical_encode_server_obj_nested_v1', {
      mode: 'server',
      server_time: 14,
      root: {
        id: [1, 10],
        kind: 'obj',
        entries: [
          {key: 'a', value: {id: [1, 11], kind: 'con', value: 1}},
          {
            key: 'b',
            value: {
              id: [1, 12],
              kind: 'arr',
              chunks: [{id: [1, 13], values: [{id: [1, 14], kind: 'con', value: 2}]}],
            },
          },
        ],
      },
    }),
    buildModelCanonicalEncodeFixture('model_canonical_encode_server_vec_sparse_v1', {
      mode: 'server',
      server_time: 22,
      root: {
        id: [1, 20],
        kind: 'vec',
        values: [
          {id: [1, 21], kind: 'con', value: 'x'},
          null,
          {id: [1, 22], kind: 'con', value: true},
          null,
          {id: [1, 23], kind: 'con', value: null},
        ],
      },
    }),
  ];
}

function buildModelDiffParityFixture(name, sid, baseView, nextView) {
  const base = mkModel(baseView, sid);
  const baseBinary = base.toBinary();
  const model = Model.load(baseBinary, sid);
  const patch = model.api.diff(nextView);

  if (!patch) {
    return baseFixture(name, 'model_diff_parity', {
      base_model_binary_hex: hex(baseBinary),
      next_view_json: nextView,
      sid,
    }, {
      patch_present: false,
      view_after_apply_json: normalizeView(model.view()),
      model_binary_after_apply_hex: hex(model.toBinary()),
    });
  }

  const patchBinary = patch.toBinary();
  const decoded = Patch.fromBinary(patchBinary);
  const patchId = decoded.getId();
  model.applyPatch(patch);

  return baseFixture(name, 'model_diff_parity', {
    base_model_binary_hex: hex(baseBinary),
    next_view_json: nextView,
    sid,
  }, {
    patch_present: true,
    patch_binary_hex: hex(patchBinary),
    patch_op_count: decoded.ops.length,
    patch_opcodes: decoded.ops.map((op) => OPCODE_BY_NAME[op.name()]),
    patch_span: decoded.span(),
    patch_id_sid: patchId ? patchId.sid : null,
    patch_id_time: patchId ? patchId.time : null,
    patch_next_time: decoded.nextTime(),
    view_after_apply_json: normalizeView(model.view()),
    model_binary_after_apply_hex: hex(model.toBinary()),
  });
}

function allModelDiffParityFixtures() {
  const fixtures = [];
  const sidBase = 77000;
  const base = {};
  const deterministicCases = [
    {},
    {},
    {},
    {},
    {},
    {a: 1},
    {a: 1, b: 2},
    {b: 2, a: 1},
    {title: 'hello'},
    {title: 'hello world'},
    {txt: ''},
    {txt: 'abc'},
    {txt: 'The quick brown fox'},
    {arr: []},
    {arr: [1]},
    {arr: [1, 2, 3]},
    {arr: ['a', 'b', 'c']},
    {arr: [{id: 1}, {id: 2}]},
    {obj: {nested: true}},
    {obj: {nested: {level: 2}}},
    {obj: {nested: {level: 2, tags: ['x', 'y']}}},
    {n: null},
    {n: false},
    {n: 0},
    {n: -1},
    {n: 123.5},
    {user: {id: 'u1', name: 'Ada', active: true}},
    {doc: {id: 'd1', title: 'T', body: 'B', tags: ['x']}},
    {doc: {id: 'd2', title: 'T2', body: 'B2', tags: ['x', 'y']}},
    {meta: {k1: 1, k2: 2, k3: 3}},
    {meta: {k3: 3, k2: 2, k1: 1}},
    {root: [1, {a: 1}, ['x', 'y']]},
    {root: {deep: {a: {b: {c: 1}}}}},
    {root: {deep: {a: {b: {c: 2}}}}},
  ];

  for (let i = 0; i < deterministicCases.length; i++) {
    const sid = sidBase + i + 1;
    fixtures.push(
      buildModelDiffParityFixture(
        `model_diff_parity_${String(i + 1).padStart(2, '0')}_det_v1`,
        sid,
        base,
        deterministicCases[i],
      ),
    );
  }

  // Deterministic randomized corpus, still rooted from empty object for
  // stable runtime apply behavior.
  const rng = mulberry32(0x4d34f000);
  for (let i = 0; i < 30; i++) {
    const sid = sidBase + deterministicCases.length + i + 1;
    const value = randJson(rng, 4);
    const nextView = {doc: value};
    fixtures.push(
      buildModelDiffParityFixture(
        `model_diff_parity_${String(deterministicCases.length + i + 1).padStart(2, '0')}_rnd_v1`,
        sid,
        base,
        nextView,
      ),
    );
  }

  // Expanded non-empty-base corpus to avoid overfitting empty-root transitions.
  for (let i = 0; i < 116; i++) {
    const sid = sidBase + deterministicCases.length + 30 + i + 1;
    const baseView = {
      a: i,
      b: i + 1,
      t: `v${i}`,
      flag: i % 2 === 0,
    };
    const nextView = {
      a: i + 1,
      b: i + 1,
      t: `v${i}-next`,
      flag: i % 3 === 0,
      c: i * 2,
    };
    fixtures.push(
      buildModelDiffParityFixture(
        `model_diff_parity_${String(deterministicCases.length + 30 + i + 1).padStart(2, '0')}_nonempty_v1`,
        sid,
        baseView,
        nextView,
      ),
    );
  }

  return fixtures;
}

function buildModelDiffDstKeysFixture(name, sid, baseView, dstKeysView) {
  const base = mkModel(baseView, sid);
  const baseBinary = base.toBinary();
  const model = Model.load(baseBinary, sid);
  const differ = new JsonCrdtDiff(model);
  const rootObj = model.api.obj().node;
  const patch = differ.diffDstKeys(rootObj, dstKeysView);

  if (!patch || patch.ops.length === 0) {
    return baseFixture(name, 'model_diff_dst_keys', {
      base_model_binary_hex: hex(baseBinary),
      dst_keys_view_json: dstKeysView,
      sid,
    }, {
      patch_present: false,
      view_after_apply_json: normalizeView(model.view()),
      model_binary_after_apply_hex: hex(model.toBinary()),
    });
  }

  const patchBinary = patch.toBinary();
  const decoded = Patch.fromBinary(patchBinary);
  const patchId = decoded.getId();
  model.applyPatch(patch);

  return baseFixture(name, 'model_diff_dst_keys', {
    base_model_binary_hex: hex(baseBinary),
    dst_keys_view_json: dstKeysView,
    sid,
  }, {
    patch_present: true,
    patch_binary_hex: hex(patchBinary),
    patch_op_count: decoded.ops.length,
    patch_opcodes: decoded.ops.map((op) => OPCODE_BY_NAME[op.name()]),
    patch_span: decoded.span(),
    patch_id_sid: patchId ? patchId.sid : null,
    patch_id_time: patchId ? patchId.time : null,
    patch_next_time: decoded.nextTime(),
    view_after_apply_json: normalizeView(model.view()),
    model_binary_after_apply_hex: hex(model.toBinary()),
  });
}

function allModelDiffDstKeysFixtures() {
  const fixtures = [];
  const cases = [
    [{a: 1, b: 2}, {a: 9}],
    [{a: 1, b: 2}, {b: 3}],
    [{doc: {title: 'a', body: 'b'}, n: 1}, {doc: {title: 'A', body: 'b'}}],
    [{txt: 'abc', meta: {x: 1}}, {txt: 'abZc'}],
    [{arr: [1, 2], score: 1}, {arr: [1, 2, 3]}],
    [{arr: [1, 2, 3], score: 1}, {arr: [1, 3]}],
    [{obj: {x: 1, y: 2}, flag: true}, {obj: {x: 1, y: 3}}],
    [{obj: {x: 1, y: 2}, flag: true}, {flag: false}],
    [{name: 'ab', list: [1], ok: true}, {name: 'aZb', list: [1, 2]}],
    [{deep: {a: {b: 1}}, c: 1}, {deep: {a: {b: 2}}}],
    [{a: 0, b: 1, c: 2}, {a: 1, c: 3}],
    [{title: 'x', body: 'y', score: 1}, {title: 'xy'}],
    [{title: 'xy', body: 'y', score: 1}, {title: 'x'}],
    [{list: [1], keep: true}, {list: [1, 2]}],
    [{list: [1, 2], keep: true}, {list: [2]}],
    [{obj: {k: 1}, keep: 'v'}, {obj: {k: 2}}],
    [{obj: {k: 1}, keep: 'v'}, {obj: {k: 1, z: 3}}],
    [{n: null, keep: 1}, {n: 1}],
    [{n: 1, keep: 1}, {n: null}],
    [{flag: false, keep: 1}, {flag: true}],
  ];
  let idx = 1;
  for (const [base, dst] of cases) {
    fixtures.push(
      buildModelDiffDstKeysFixture(
        `model_diff_dst_keys_${String(idx).padStart(2, '0')}_v1`,
        77500 + idx,
        base,
        dst,
      ),
    );
    idx++;
  }
  // Expanded deterministic mirror corpus to raise floor without introducing
  // non-deterministic parity drift in dst-keys ordering behavior.
  for (const [base, dst] of cases) {
    fixtures.push(
      buildModelDiffDstKeysFixture(
        `model_diff_dst_keys_${String(idx).padStart(2, '0')}_mirror_v1`,
        77600 + idx,
        cloneJson(base),
        cloneJson(dst),
      ),
    );
    idx++;
  }
  return fixtures;
}

function normalizeView(view) {
  return view === undefined ? null : view;
}

function bytesRecordToHexRecord(record) {
  const out = {};
  for (const [k, v] of Object.entries(record)) {
    out[k] = hex(v);
  }
  return out;
}

function hexRecordToBytesRecord(record) {
  const out = {};
  for (const [k, v] of Object.entries(record)) {
    out[k] = fromHex(v);
  }
  return out;
}

function mustPatch(model, next) {
  const patch = model.api.diff(next);
  if (!patch) throw new Error('expected non-empty patch');
  model.applyPatch(patch);
  return patch;
}

function buildApplyReplayFixture(name, baseModelBinary, patches, replayPattern, label) {
  const model = Model.fromBinary(baseModelBinary);
  let effective = 0;
  for (const idx of replayPattern) {
    const before = hex(model.toBinary());
    model.applyPatch(patches[idx]);
    const after = hex(model.toBinary());
    if (after !== before) effective++;
  }
  const patchIds = patches.map((p) => {
    const id = p.getId();
    return id ? [id.sid, id.time] : null;
  });
  return baseFixture(name, 'model_apply_replay', {
    base_model_binary_hex: hex(baseModelBinary),
    patches_binary_hex: patches.map((p) => hex(p.toBinary())),
    replay_pattern: replayPattern,
    label,
  }, {
    view_json: normalizeView(model.view()),
    model_binary_hex: hex(model.toBinary()),
    applied_patch_count_effective: effective,
    clock_observed: {
      patch_ids: patchIds,
    },
  });
}

function allModelApplyReplayFixtures() {
  const fixtures = [];

  // Minimal base model for deterministic replay semantics.
  const base = Model.create(undefined, 75001);
  base.api.set({});
  base.api.flush();
  const baseBin = base.toBinary();

  const mkPatch = (ops) => {
    const p = new Patch();
    p.ops.push(...ops);
    return p;
  };

  // Object stream
  const obj1 = mkPatch([
    new NewObjOp(ts(76100, 1)),
    new InsValOp(ts(76100, 2), ts(0, 0), ts(76100, 1)),
    new NewConOp(ts(76100, 3), 'A'),
    new InsObjOp(ts(76100, 4), ts(76100, 1), [['title', ts(76100, 3)]]),
  ]);
  const obj2 = mkPatch([
    new NewConOp(ts(76100, 5), 'AA'),
    new InsObjOp(ts(76100, 6), ts(76100, 1), [['title', ts(76100, 5)]]),
  ]);
  const objPeer = mkPatch([
    new NewConOp(ts(76110, 1), 'B'),
    new InsObjOp(ts(76110, 2), ts(76100, 1), [['body', ts(76110, 1)]]),
  ]);

  // Array stream
  const arr1 = mkPatch([
    new NewArrOp(ts(76200, 1)),
    new InsValOp(ts(76200, 2), ts(0, 0), ts(76200, 1)),
    new NewConOp(ts(76200, 3), 1),
    new NewConOp(ts(76200, 4), 2),
    new InsArrOp(ts(76200, 5), ts(76200, 1), ts(76200, 1), [ts(76200, 3), ts(76200, 4)]),
  ]);
  const arr2 = mkPatch([
    new NewConOp(ts(76200, 6), 9),
    new UpdArrOp(ts(76200, 7), ts(76200, 1), ts(76200, 5), ts(76200, 6)),
  ]);
  const arr3 = mkPatch([new DelOp(ts(76200, 8), ts(76200, 1), [tss(76200, 6, 1)])]);

  // String stream
  const str1 = mkPatch([
    new NewStrOp(ts(76300, 1)),
    new InsValOp(ts(76300, 2), ts(0, 0), ts(76300, 1)),
    new InsStrOp(ts(76300, 3), ts(76300, 1), ts(76300, 1), 'abc'),
  ]);
  const str2 = mkPatch([new DelOp(ts(76300, 6), ts(76300, 1), [tss(76300, 4, 1)])]);
  const str3 = mkPatch([new InsStrOp(ts(76300, 7), ts(76300, 1), ts(76300, 5), 'Z')]);

  // Binary stream
  const bin1 = mkPatch([
    new NewBinOp(ts(76400, 1)),
    new InsValOp(ts(76400, 2), ts(0, 0), ts(76400, 1)),
    new InsBinOp(ts(76400, 3), ts(76400, 1), ts(76400, 1), new Uint8Array([1, 2, 3])),
  ]);
  const bin2 = mkPatch([new DelOp(ts(76400, 6), ts(76400, 1), [tss(76400, 4, 1)])]);
  const bin3 = mkPatch([
    new InsBinOp(ts(76400, 7), ts(76400, 1), ts(76400, 5), new Uint8Array([9])),
  ]);

  // Vec stream
  const vec1 = mkPatch([
    new NewVecOp(ts(76500, 1)),
    new InsValOp(ts(76500, 2), ts(0, 0), ts(76500, 1)),
    new NewConOp(ts(76500, 3), 7),
    new NewConOp(ts(76500, 4), 'x'),
    new InsVecOp(ts(76500, 5), ts(76500, 1), [[0, ts(76500, 3)], [2, ts(76500, 4)]]),
  ]);
  const vec2 = mkPatch([
    new NewConOp(ts(76500, 6), 8),
    new InsVecOp(ts(76500, 7), ts(76500, 1), [[1, ts(76500, 6)]]),
  ]);
  const vecPeer = mkPatch([
    new NewConOp(ts(76510, 1), false),
    new InsVecOp(ts(76510, 2), ts(76500, 1), [[2, ts(76510, 1)]]),
  ]);

  const groups = [
    {name: 'obj', patches: [obj1, obj2, objPeer]},
    {name: 'arr', patches: [arr1, arr2, arr3]},
    {name: 'str', patches: [str1, str2, str3]},
    {name: 'bin', patches: [bin1, bin2, bin3]},
    {name: 'vec', patches: [vec1, vec2, vecPeer]},
  ];

  const patterns = [
    {name: 'dup_single', replay: [0, 0]},
    {name: 'dup_batch', replay: [0, 1, 0, 1]},
    {name: 'stale_order', replay: [1, 0]},
    {name: 'in_order', replay: [0, 1, 2]},
    {name: 'out_of_order', replay: [2, 0, 1]},
    {name: 'interleaved_dup', replay: [0, 2, 0, 1, 2]},
    {name: 'dup_tail', replay: [0, 1, 2, 2, 2]},
    {name: 'dup_head', replay: [0, 0, 0, 1, 2]},
    {name: 'dup_middle', replay: [0, 1, 1, 1, 2]},
    {name: 'full_cycle_twice', replay: [0, 1, 2, 0, 1, 2]},
    {name: 'late_duplicate_only', replay: [0, 1, 2, 1]},
    {name: 'peer_first_duplicate', replay: [2, 2, 0, 1]},
    {name: 'peer_only_duplicate', replay: [2, 2, 2]},
    {name: 'reverse_then_forward', replay: [2, 1, 0, 0, 1, 2]},
    {name: 'head_tail_duplicates', replay: [0, 0, 1, 2, 2]},
    {name: 'skip_middle_then_replay', replay: [0, 2, 1, 1]},
    {name: 'triple_cycle', replay: [0, 1, 2, 0, 1, 2, 0, 1, 2]},
    {name: 'mixed_peer_reapply', replay: [2, 0, 2, 1, 2]},
  ];

  let idx = 1;
  for (const g of groups) {
    for (const p of patterns) {
      fixtures.push(
        buildApplyReplayFixture(
          `model_apply_replay_${String(idx).padStart(2, '0')}_${g.name}_${p.name}_v1`,
          baseBin,
          g.patches,
          p.replay,
          `${g.name}_${p.name}`,
        ),
      );
      idx++;
    }
  }

  return fixtures;
}

function appendPatchLog(existing, patchBinary) {
  if (!existing || existing.length === 0) {
    const out = new Uint8Array(1 + 4 + patchBinary.length);
    out[0] = 1;
    const view = new DataView(out.buffer, out.byteOffset, out.byteLength);
    view.setUint32(1, patchBinary.length);
    out.set(patchBinary, 5);
    return out;
  }
  const out = new Uint8Array(existing.length + 4 + patchBinary.length);
  out.set(existing, 0);
  const view = new DataView(out.buffer, out.byteOffset, out.byteLength);
  view.setUint32(existing.length, patchBinary.length);
  out.set(patchBinary, existing.length + 4);
  return out;
}

function decodePatchLogCount(data) {
  if (!data || data.length === 0) return 0;
  if (data[0] !== 1) throw new Error('Unsupported patch log version');
  let offset = 1;
  let count = 0;
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  while (offset < data.length) {
    if (offset + 4 > data.length) throw new Error('Corrupt pending patches: truncated length header');
    const len = view.getUint32(offset);
    offset += 4;
    if (offset + len > data.length) throw new Error('Corrupt pending patches: truncated patch data');
    offset += len;
    count++;
  }
  return count;
}

function buildLessdbCreateDiffApplyFixture(name, sid, initial, next) {
  const model = mkModel(initial, sid);
  const baseModelBinary = model.toBinary();
  const loaded = Model.load(baseModelBinary, sid);
  const patch = loaded.api.diff(next);

  let pending = new Uint8Array(0);
  const steps = [];
  steps.push({
    kind: 'diff',
    patch_present: !!patch,
    patch_binary_hex: patch ? hex(patch.toBinary()) : null,
  });

  if (patch) {
    const patchBinary = patch.toBinary();
    const decoded = Patch.fromBinary(patchBinary);
    const patchId = decoded.getId();
    loaded.applyPatch(patch);
    pending = appendPatchLog(pending, patchBinary);

    steps[0].patch_op_count = decoded.ops.length;
    steps[0].patch_opcodes = decoded.ops.map((op) => OPCODE_BY_NAME[op.name()]);
    steps[0].patch_span = decoded.span();
    steps[0].patch_id_sid = patchId ? patchId.sid : null;
    steps[0].patch_id_time = patchId ? patchId.time : null;
    steps[0].patch_next_time = decoded.nextTime();
  }

  steps.push({
    kind: 'apply_last_diff',
    view_json: normalizeView(loaded.view()),
    model_binary_hex: hex(loaded.toBinary()),
  });
  steps.push({
    kind: 'patch_log_append_last_diff',
    pending_patch_log_hex: hex(pending),
  });
  steps.push({
    kind: 'patch_log_deserialize',
    patch_count: decodePatchLogCount(pending),
  });

  return baseFixture(name, 'lessdb_model_manager', {
    workflow: 'create_diff_apply',
    sid,
    initial_json: initial,
    ops: [
      {kind: 'diff', next_view_json: next},
      {kind: 'apply_last_diff'},
      {kind: 'patch_log_append_last_diff'},
      {kind: 'patch_log_deserialize'},
    ],
  }, {
    steps,
    final_view_json: normalizeView(loaded.view()),
    final_model_binary_hex: hex(loaded.toBinary()),
    final_pending_patch_log_hex: hex(pending),
  });
}

function buildLessdbForkMergeFixture(name, sid, initial, forkSid, nextFork) {
  const base = mkModel(initial, sid);
  const baseBinary = base.toBinary();
  const fork = Model.fromBinary(baseBinary).fork(forkSid);
  const patch = fork.api.diff(nextFork);
  if (!patch) throw new Error('expected non-empty patch for fork fixture');
  const patchBinary = patch.toBinary();
  fork.applyPatch(patch);

  const merged = Model.fromBinary(baseBinary);
  merged.applyPatch(Patch.fromBinary(patchBinary));

  return baseFixture(name, 'lessdb_model_manager', {
    workflow: 'fork_merge',
    sid,
    initial_json: initial,
    ops: [
      {kind: 'fork', sid: forkSid},
      {kind: 'diff_on_fork', next_view_json: nextFork},
      {kind: 'apply_last_diff_on_fork'},
      {kind: 'merge_into_base'},
    ],
  }, {
    steps: [
      {kind: 'fork', view_json: normalizeView(Model.fromBinary(baseBinary).fork(forkSid).view())},
      {kind: 'diff_on_fork', patch_present: true, patch_binary_hex: hex(patchBinary)},
      {kind: 'apply_last_diff_on_fork', view_json: normalizeView(fork.view()), model_binary_hex: hex(fork.toBinary())},
      {kind: 'merge_into_base', view_json: normalizeView(merged.view()), model_binary_hex: hex(merged.toBinary())},
    ],
    final_view_json: normalizeView(merged.view()),
    final_model_binary_hex: hex(merged.toBinary()),
  });
}

function buildLessdbMergeIdempotentFixture(name, sid, initial, next) {
  const base = mkModel(initial, sid);
  const baseBinary = base.toBinary();
  const local = Model.load(baseBinary, sid);
  const patch = local.api.diff(next);
  if (!patch) throw new Error('expected non-empty patch for merge fixture');
  const patchBinary = patch.toBinary();
  local.applyPatch(patch);

  const remote = Model.fromBinary(baseBinary);
  const parsed = Patch.fromBinary(patchBinary);
  remote.applyPatch(parsed);
  remote.applyPatch(parsed);

  return baseFixture(name, 'lessdb_model_manager', {
    workflow: 'merge_idempotent',
    sid,
    base_model_binary_hex: hex(baseBinary),
    ops: [
      {kind: 'merge', patches_binary_hex: [hex(patchBinary), hex(patchBinary)]},
    ],
  }, {
    steps: [
      {kind: 'merge', view_json: normalizeView(remote.view()), model_binary_hex: hex(remote.toBinary())},
    ],
    patch_binary_hex: hex(patchBinary),
    final_view_json: normalizeView(remote.view()),
    final_model_binary_hex: hex(remote.toBinary()),
  });
}

function allLessdbModelManagerFixtures() {
  const fixtures = [];
  let idx = 1;

  const createCases = [
    [{}, {a: 1}],
    [{a: 1}, {a: 2}],
    [{a: 1}, {a: 1}],
    [{txt: 'a'}, {txt: 'ab'}],
    [{txt: 'abc'}, {txt: 'ac'}],
    [{arr: [1, 2]}, {arr: [1, 2, 3]}],
    [{arr: [1, 2, 3]}, {arr: [1, 3]}],
    [{obj: {x: 1}}, {obj: {x: 1, y: 2}}],
    [{obj: {x: 1, y: 2}}, {obj: {x: 1}}],
    [{doc: {title: 'a', body: 'b'}}, {doc: {title: 'A', body: 'b'}}],
    [{doc: {title: 'a', body: 'b'}}, {doc: {title: 'a', body: 'B'}}],
    [{n: null}, {n: 1}],
    [{n: 1}, {n: null}],
    [{meta: {a: 1, b: 2}}, {meta: {b: 2, a: 1}}],
    [{items: [{id: 1}, {id: 2}]}, {items: [{id: 1}, {id: 3}]}],
    [{root: {deep: {v: 'x'}}}, {root: {deep: {v: 'y'}}}],
    [{score: 1}, {score: 1}],
    [{tags: ['a']}, {tags: ['a', 'b']}],
    [{tags: ['a', 'b']}, {tags: ['b']}],
    [{flag: true}, {flag: false}],
    [{a: 10}, {a: 11}],
    [{title: 'x'}, {title: 'xy'}],
    [{title: 'xy'}, {title: 'x'}],
    [{arr: [1]}, {arr: [1, 2]}],
    [{arr: [1, 2]}, {arr: [2]}],
    [{obj: {x: 1}}, {obj: {x: 2}}],
    [{obj: {x: 1}}, {obj: {x: 1, z: 3}}],
    [{n: 0}, {n: 1}],
    [{n: 1}, {n: 0}],
    [{flag: false}, {flag: true}],
    [{tags: ['x']}, {tags: ['x', 'y']}],
    [{tags: ['x', 'y']}, {tags: ['y']}],
    [{doc: {title: 'm', body: 'n'}}, {doc: {title: 'M', body: 'n'}}],
    [{doc: {title: 'm', body: 'n'}}, {doc: {title: 'm', body: 'N'}}],
    [{score: 3}, {score: 3}],
  ];

  for (const [initial, next] of createCases) {
    fixtures.push(
      buildLessdbCreateDiffApplyFixture(
        `lessdb_model_manager_${String(idx).padStart(2, '0')}_create_diff_apply_v1`,
        78000 + idx,
        initial,
        next,
      ),
    );
    idx++;
  }

  const forkCases = [
    [{title: 'a'}, {title: 'A'}],
    [{body: 'x'}, {body: 'xy'}],
    [{arr: [1]}, {arr: [1, 2]}],
    [{obj: {k: 1}}, {obj: {k: 2}}],
    [{txt: 'abc'}, {txt: 'abZc'}],
    [{title: 'b'}, {title: 'B'}],
    [{body: 'q'}, {body: 'q!'}],
    [{arr: [2]}, {arr: [2, 3]}],
    [{obj: {k: 3}}, {obj: {k: 4}}],
    [{txt: 'aba'}, {txt: 'abZa'}],
  ];
  for (const [initial, nextFork] of forkCases) {
    fixtures.push(
      buildLessdbForkMergeFixture(
        `lessdb_model_manager_${String(idx).padStart(2, '0')}_fork_merge_v1`,
        78000 + idx,
        initial,
        88000 + idx,
        nextFork,
      ),
    );
    idx++;
  }

  const mergeCases = [
    [{title: 'a'}, {title: 'A'}],
    [{score: 1}, {score: 2}],
    [{arr: [1, 2]}, {arr: [2, 1]}],
    [{txt: 'abc'}, {txt: 'axbc'}],
    [{obj: {a: 1}}, {obj: {a: 1, b: 2}}],
    [{title: 'b'}, {title: 'B'}],
    [{score: 2}, {score: 3}],
    [{arr: [3, 4]}, {arr: [4, 3]}],
    [{txt: 'hello'}, {txt: 'hello!'}],
    [{obj: {a: 2}}, {obj: {a: 2, b: 3}}],
  ];
  for (const [initial, next] of mergeCases) {
    fixtures.push(
      buildLessdbMergeIdempotentFixture(
        `lessdb_model_manager_${String(idx).padStart(2, '0')}_merge_idempotent_v1`,
        78000 + idx,
        initial,
        next,
      ),
    );
    idx++;
  }

  return fixtures;
}

function toPathArray(path) {
  return path.map((p) => (typeof p === 'number' ? p : String(p)));
}

function findAtPath(value, path) {
  let cur = value;
  for (const step of path) {
    if (Array.isArray(cur)) {
      const idx = typeof step === 'number' ? step : Number(step);
      if (!Number.isInteger(idx) || idx < 0 || idx >= cur.length) return undefined;
      cur = cur[idx];
      continue;
    }
    if (cur && typeof cur === 'object') {
      const key = String(step);
      if (!Object.prototype.hasOwnProperty.call(cur, key)) return undefined;
      cur = cur[key];
      continue;
    }
    return undefined;
  }
  return cur;
}

function setAtPath(root, path, value) {
  if (!path.length) return value;
  let cur = root;
  for (let i = 0; i < path.length - 1; i++) {
    const step = path[i];
    if (Array.isArray(cur)) {
      const idx = typeof step === 'number' ? step : Number(step);
      if (!Number.isInteger(idx) || idx < 0 || idx >= cur.length) throw new Error('invalid array path');
      cur = cur[idx];
    } else if (cur && typeof cur === 'object') {
      const key = String(step);
      if (!Object.prototype.hasOwnProperty.call(cur, key)) throw new Error('missing object path');
      cur = cur[key];
    } else throw new Error('non-container in path');
  }
  const last = path[path.length - 1];
  if (Array.isArray(cur)) {
    const idx = typeof last === 'number' ? last : Number(last);
    if (!Number.isInteger(idx) || idx < 0 || idx >= cur.length) throw new Error('invalid array leaf path');
    cur[idx] = value;
    return root;
  }
  if (cur && typeof cur === 'object') {
    cur[String(last)] = value;
    return root;
  }
  throw new Error('invalid leaf parent');
}

function buildModelApiWorkflowFixture(name, sid, initial, ops) {
  const model = mkModel(initial, sid);
  const baseBinary = model.toBinary();
  const runtime = mkModel(initial, sid);
  runtime.api.flush();
  let currentView = cloneJson(initial);

  const inputOps = [];
  const expectedSteps = [];

  for (const op of ops) {
    if (op.kind === 'find') {
      const path = toPathArray(op.path);
      const actual = findAtPath(runtime.view(), op.path);
      inputOps.push({kind: 'find', path});
      expectedSteps.push({kind: 'find', path, value_json: normalizeView(actual)});
      continue;
    }
    if (op.kind === 'set') {
      currentView = setAtPath(currentView, op.path, cloneJson(op.value));
      const patch = runtime.api.diff(currentView);
      if (patch) runtime.applyPatch(patch);
      inputOps.push({kind: 'set', path: toPathArray(op.path), value_json: op.value});
      expectedSteps.push({kind: 'set', view_json: normalizeView(runtime.view())});
      continue;
    }
    if (op.kind === 'add') {
      if (op.path.length === 0) throw new Error('add path must not be empty');
      const parentPath = op.path.slice(0, -1);
      const leaf = op.path[op.path.length - 1];
      const parent = findAtPath(currentView, parentPath);
      if (Array.isArray(parent)) {
        const idx = Math.max(0, Math.min(Number(leaf), parent.length));
        parent.splice(idx, 0, cloneJson(op.value));
      } else if (parent && typeof parent === 'object') {
        parent[String(leaf)] = cloneJson(op.value);
      } else {
        throw new Error('add parent is not container');
      }
      const patch = runtime.api.diff(currentView);
      if (patch) runtime.applyPatch(patch);
      inputOps.push({kind: 'add', path: toPathArray(op.path), value_json: op.value});
      expectedSteps.push({kind: 'add', view_json: normalizeView(runtime.view())});
      continue;
    }
    if (op.kind === 'replace') {
      currentView = setAtPath(currentView, op.path, cloneJson(op.value));
      const patch = runtime.api.diff(currentView);
      if (patch) runtime.applyPatch(patch);
      inputOps.push({kind: 'replace', path: toPathArray(op.path), value_json: op.value});
      expectedSteps.push({kind: 'replace', view_json: normalizeView(runtime.view())});
      continue;
    }
    if (op.kind === 'remove') {
      if (op.path.length === 0) throw new Error('remove path must not be empty');
      const parentPath = op.path.slice(0, -1);
      const leaf = op.path[op.path.length - 1];
      const parent = findAtPath(currentView, parentPath);
      if (Array.isArray(parent)) {
        const idx = Number(leaf);
        if (Number.isInteger(idx) && idx >= 0 && idx < parent.length) parent.splice(idx, 1);
      } else if (parent && typeof parent === 'object') {
        delete parent[String(leaf)];
      } else {
        throw new Error('remove parent is not container');
      }
      const patch = runtime.api.diff(currentView);
      if (patch) runtime.applyPatch(patch);
      inputOps.push({kind: 'remove', path: toPathArray(op.path)});
      expectedSteps.push({kind: 'remove', view_json: normalizeView(runtime.view())});
      continue;
    }
    if (op.kind === 'obj_put') {
      const obj = findAtPath(currentView, op.path);
      if (!obj || typeof obj !== 'object' || Array.isArray(obj)) throw new Error('obj_put path is not object');
      obj[String(op.key)] = cloneJson(op.value);
      const patch = runtime.api.diff(currentView);
      if (patch) runtime.applyPatch(patch);
      inputOps.push({
        kind: 'obj_put',
        path: toPathArray(op.path),
        key: String(op.key),
        value_json: op.value,
      });
      expectedSteps.push({kind: 'obj_put', view_json: normalizeView(runtime.view())});
      continue;
    }
    if (op.kind === 'arr_push') {
      const arr = findAtPath(currentView, op.path);
      if (!Array.isArray(arr)) throw new Error('arr_push path is not array');
      arr.push(cloneJson(op.value));
      const patch = runtime.api.diff(currentView);
      if (patch) runtime.applyPatch(patch);
      inputOps.push({kind: 'arr_push', path: toPathArray(op.path), value_json: op.value});
      expectedSteps.push({kind: 'arr_push', view_json: normalizeView(runtime.view())});
      continue;
    }
    if (op.kind === 'str_ins') {
      const s = findAtPath(currentView, op.path);
      if (typeof s !== 'string') throw new Error('str_ins path is not string');
      const chars = Array.from(s);
      const pos = Math.max(0, Math.min(op.pos, chars.length));
      chars.splice(pos, 0, ...Array.from(op.text));
      currentView = setAtPath(currentView, op.path, chars.join(''));
      const patch = runtime.api.diff(currentView);
      if (patch) runtime.applyPatch(patch);
      inputOps.push({kind: 'str_ins', path: toPathArray(op.path), pos: op.pos, text: op.text});
      expectedSteps.push({kind: 'str_ins', view_json: normalizeView(runtime.view())});
      continue;
    }
    if (op.kind === 'apply_batch') {
      const patchHexes = [];
      for (const nextView of op.batch_next_views) {
        const patch = runtime.api.diff(nextView);
        if (patch) {
          patchHexes.push(hex(patch.toBinary()));
          runtime.applyPatch(patch);
        }
      }
      inputOps.push({kind: 'apply_batch', patches_binary_hex: patchHexes});
      expectedSteps.push({kind: 'apply_batch', view_json: normalizeView(runtime.view())});
      continue;
    }
    throw new Error(`unsupported model_api op: ${op.kind}`);
  }

  return baseFixture(name, 'model_api_workflow', {
    sid,
    initial_json: cloneJson(initial),
    base_model_binary_hex: hex(baseBinary),
    ops: inputOps,
  }, {
    steps: expectedSteps,
    final_view_json: normalizeView(runtime.view()),
    final_model_binary_hex: hex(runtime.toBinary()),
  });
}

function allModelApiWorkflowFixtures() {
  const fixtures = [];
  let idx = 1;
  const cases = [
    {
      initial: {doc: {title: 'a', items: [1]}},
      ops: [
        {kind: 'find', path: ['doc', 'title']},
        {kind: 'add', path: ['doc', 'subtitle'], value: 's'},
        {kind: 'replace', path: ['doc', 'subtitle'], value: 'S'},
        {kind: 'obj_put', path: ['doc'], key: 'flag', value: true},
        {kind: 'arr_push', path: ['doc', 'items'], value: 2},
        {kind: 'remove', path: ['doc', 'subtitle']},
        {kind: 'set', path: ['doc', 'title'], value: 'A'},
      ],
    },
    {
      initial: {name: 'ab', list: [1, 2]},
      ops: [
        {kind: 'str_ins', path: ['name'], pos: 1, text: 'Z'},
        {kind: 'add', path: ['list', 1], value: 9},
        {kind: 'replace', path: ['list', 0], value: 7},
        {kind: 'remove', path: ['list', 2]},
        {kind: 'find', path: ['name']},
        {kind: 'arr_push', path: ['list'], value: 3},
      ],
    },
    {
      initial: {meta: {v: 1}, tags: ['x']},
      ops: [
        {kind: 'obj_put', path: ['meta'], key: 'ok', value: false},
        {kind: 'find', path: ['meta', 'ok']},
        {kind: 'add', path: ['meta', 'note'], value: 'n'},
        {kind: 'replace', path: ['meta', 'v'], value: 2},
        {kind: 'remove', path: ['meta', 'note']},
      ],
    },
  ];

  for (const c of cases) {
    fixtures.push(
      buildModelApiWorkflowFixture(
        `model_api_workflow_${String(idx).padStart(2, '0')}_v1`,
        79000 + idx,
        c.initial,
        c.ops,
      ),
    );
    idx++;
  }

  const rng = mulberry32(0x6d6f6465);
  for (let i = 0; i < 57; i++) {
    const initial = {
      doc: {
        seed: randJson(rng, 2),
        nested: {v: randInt(rng, 50)},
      },
      title: randString(rng, 1, 8),
      list: [randInt(rng, 9)],
    };
    const ops = [
      {kind: 'find', path: ['title']},
      {kind: 'obj_put', path: ['doc'], key: `k${i}`, value: randScalar(rng)},
      {kind: 'add', path: ['doc', `n${i}`], value: randScalar(rng)},
      {kind: 'replace', path: ['list', 0], value: randInt(rng, 100)},
      {kind: 'remove', path: ['doc', 'seed']},
      {kind: 'arr_push', path: ['list'], value: randInt(rng, 100)},
      {kind: 'str_ins', path: ['title'], pos: randInt(rng, 3), text: 'x'},
      {kind: 'set', path: ['doc'], value: randJson(rng, 2)},
    ];
    fixtures.push(
      buildModelApiWorkflowFixture(
        `model_api_workflow_${String(idx).padStart(2, '0')}_v1`,
        79000 + idx,
        initial,
        ops,
      ),
    );
    idx++;
  }

  return fixtures;
}

function buildModelLifecycleFixture(name, sid, initial, nextA, nextB, workflow, loadSid) {
  const base = mkModel(initial, sid);
  const baseBinary = base.toBinary();

  const step1Model = Model.load(baseBinary, sid);
  const pA = step1Model.api.diff(nextA);
  if (!pA) throw new Error('expected non-empty step1 diff patch');
  step1Model.applyPatch(pA);
  const pB = step1Model.api.diff(nextB);
  if (!pB) throw new Error('expected non-empty step2 diff patch');

  const seedPatch = Model.load(mkModel({}, sid).toBinary(), sid).api.diff(initial);
  if (!seedPatch) throw new Error('expected non-empty seed patch');

  const seedHex = hex(seedPatch.toBinary());
  const batchHex = [hex(pA.toBinary()), hex(pB.toBinary())];

  let model;
  if (workflow === 'from_patches_apply_batch') {
    model = Model.fromPatches([Patch.fromBinary(fromHex(seedHex))]);
  } else if (workflow === 'load_apply_batch') {
    model = Model.load(baseBinary, loadSid);
  } else {
    throw new Error(`unsupported lifecycle workflow: ${workflow}`);
  }
  for (const ph of batchHex) model.applyPatch(Patch.fromBinary(fromHex(ph)));

  return baseFixture(name, 'model_lifecycle_workflow', {
    workflow,
    sid,
    load_sid: loadSid ?? null,
    initial_json: cloneJson(initial),
    base_model_binary_hex: hex(baseBinary),
    seed_patches_binary_hex: [seedHex],
    batch_patches_binary_hex: batchHex,
  }, {
    final_view_json: normalizeView(model.view()),
    final_model_binary_hex: hex(model.toBinary()),
  });
}

function allModelLifecycleFixtures() {
  const fixtures = [];
  const cases = [
    [{doc: {title: 'a', n: 1}}, {doc: {title: 'A', n: 1}}, {doc: {title: 'A', n: 2}}],
    [{arr: [1]}, {arr: [1, 2]}, {arr: [2]}],
    [{txt: 'ab'}, {txt: 'aZb'}, {txt: 'aZb!'}],
    [{meta: {a: 1}}, {meta: {a: 2}}, {meta: {a: 2, b: 3}}],
    [{flag: false, score: 1}, {flag: true, score: 1}, {flag: true, score: 2}],
    [{doc: {items: [{id: 1}]}}, {doc: {items: [{id: 1}, {id: 2}]}}, {doc: {items: [{id: 2}]}}],
    [{doc: {a: 1, b: 2}}, {doc: {a: 2, b: 2}}, {doc: {a: 2, b: 3}}],
    [{title: 'x', list: [1]}, {title: 'xy', list: [1]}, {title: 'xy', list: [1, 2]}],
    [{title: 'xy', list: [1, 2]}, {title: 'x', list: [1, 2]}, {title: 'x', list: [2]}],
    [{meta: {ok: false}}, {meta: {ok: true}}, {meta: {ok: true, n: 1}}],
    [{arr: [1, 2, 3]}, {arr: [1, 9, 3]}, {arr: [1, 9]}],
    [{txt: 'abc'}, {txt: 'abZc'}, {txt: 'abZc!'}],
    [{obj: {k: 'a'}}, {obj: {k: 'b'}}, {obj: {k: 'b', z: true}}],
    [{score: 9, done: false}, {score: 10, done: false}, {score: 10, done: true}],
    [{doc: {items: []}}, {doc: {items: [{id: 1}]}}, {doc: {items: [{id: 1}, {id: 2}]}}],
  ];
  let idx = 1;
  for (const [initial, nextA, nextB] of cases) {
    fixtures.push(
      buildModelLifecycleFixture(
        `model_lifecycle_workflow_${String(idx).padStart(2, '0')}_from_patches_v1`,
        79500 + idx,
        initial,
        nextA,
        nextB,
        'from_patches_apply_batch',
        null,
      ),
    );
    idx++;
    fixtures.push(
      buildModelLifecycleFixture(
        `model_lifecycle_workflow_${String(idx).padStart(2, '0')}_load_v1`,
        79500 + idx,
        initial,
        nextA,
        nextB,
        'load_apply_batch',
        89600 + idx,
      ),
    );
    idx++;
  }
  return fixtures;
}

function buildModelApiProxyFanoutFixture(name, sid, initial, ops) {
  const model = mkModel(initial, sid);
  const baseBinary = model.toBinary();
  const runtime = Model.load(baseBinary, sid);
  let currentView = cloneJson(initial);
  const scopedPath = ['doc', 'title'];
  let changeCount = 0;
  let scopedCount = 0;
  const inputOps = [];
  const expectedSteps = [];

  for (const op of ops) {
    if (op.kind === 'read') {
      inputOps.push({kind: 'read', path: toPathArray(op.path)});
      expectedSteps.push({
        kind: 'read',
        value_json: normalizeView(findAtPath(runtime.view(), op.path)),
      });
      continue;
    }

    const beforeScoped = normalizeView(findAtPath(runtime.view(), scopedPath));
    if (op.kind === 'node_obj_put') {
      const obj = findAtPath(currentView, op.path);
      if (!obj || typeof obj !== 'object' || Array.isArray(obj)) throw new Error('node_obj_put path is not object');
      obj[String(op.key)] = cloneJson(op.value);
      inputOps.push({
        kind: 'node_obj_put',
        path: toPathArray(op.path),
        key: String(op.key),
        value_json: cloneJson(op.value),
      });
    } else if (op.kind === 'node_arr_push') {
      const arr = findAtPath(currentView, op.path);
      if (!Array.isArray(arr)) throw new Error('node_arr_push path is not array');
      arr.push(cloneJson(op.value));
      inputOps.push({
        kind: 'node_arr_push',
        path: toPathArray(op.path),
        value_json: cloneJson(op.value),
      });
    } else if (op.kind === 'node_str_ins') {
      const s = findAtPath(currentView, op.path);
      if (typeof s !== 'string') throw new Error('node_str_ins path is not string');
      const chars = Array.from(s);
      const pos = Math.max(0, Math.min(op.pos, chars.length));
      chars.splice(pos, 0, ...Array.from(op.text));
      currentView = setAtPath(currentView, op.path, chars.join(''));
      inputOps.push({
        kind: 'node_str_ins',
        path: toPathArray(op.path),
        pos: op.pos,
        text: op.text,
      });
    } else if (op.kind === 'node_add') {
      const parentPath = op.path.slice(0, -1);
      const leaf = op.path[op.path.length - 1];
      const parent = findAtPath(currentView, parentPath);
      if (Array.isArray(parent)) {
        const idx = Math.max(0, Math.min(Number(leaf), parent.length));
        parent.splice(idx, 0, cloneJson(op.value));
      } else if (parent && typeof parent === 'object') {
        parent[String(leaf)] = cloneJson(op.value);
      } else {
        throw new Error('node_add parent is not container');
      }
      inputOps.push({
        kind: 'node_add',
        path: toPathArray(op.path),
        value_json: cloneJson(op.value),
      });
    } else if (op.kind === 'node_replace') {
      currentView = setAtPath(currentView, op.path, cloneJson(op.value));
      inputOps.push({
        kind: 'node_replace',
        path: toPathArray(op.path),
        value_json: cloneJson(op.value),
      });
    } else if (op.kind === 'node_remove') {
      const parentPath = op.path.slice(0, -1);
      const leaf = op.path[op.path.length - 1];
      const parent = findAtPath(currentView, parentPath);
      if (Array.isArray(parent)) {
        const idx = Number(leaf);
        if (Number.isInteger(idx) && idx >= 0 && idx < parent.length) parent.splice(idx, 1);
      } else if (parent && typeof parent === 'object') {
        delete parent[String(leaf)];
      } else if (typeof parent === 'string') {
        const idx = Number(leaf);
        if (Number.isInteger(idx) && idx >= 0) {
          const chars = Array.from(parent);
          if (idx < chars.length) {
            chars.splice(idx, 1);
            currentView = setAtPath(currentView, parentPath, chars.join(''));
          }
        }
      } else {
        throw new Error('node_remove parent is not container');
      }
      inputOps.push({
        kind: 'node_remove',
        path: toPathArray(op.path),
      });
    } else {
      throw new Error(`unsupported proxy/fanout op: ${op.kind}`);
    }

    const patch = runtime.api.diff(currentView);
    if (patch) {
      runtime.applyPatch(patch);
      changeCount++;
    }
    const afterScoped = normalizeView(findAtPath(runtime.view(), scopedPath));
    if (beforeScoped !== afterScoped) scopedCount++;
    expectedSteps.push({
      kind: op.kind,
      view_json: normalizeView(runtime.view()),
    });
  }

  return baseFixture(name, 'model_api_proxy_fanout_workflow', {
    sid,
    initial_json: cloneJson(initial),
    base_model_binary_hex: hex(baseBinary),
    scoped_path: toPathArray(scopedPath),
    ops: inputOps,
  }, {
    steps: expectedSteps,
    final_view_json: normalizeView(runtime.view()),
    final_model_binary_hex: hex(runtime.toBinary()),
    fanout: {
      change_count: changeCount,
      scoped_count: scopedCount,
    },
  });
}

function allModelApiProxyFanoutFixtures() {
  const fixtures = [];
  let idx = 1;
  const deterministic = [
    {
      initial: {doc: {title: 'ab', items: [1], flag: false}},
      ops: [
        {kind: 'read', path: ['doc', 'title']},
        {kind: 'node_obj_put', path: ['doc'], key: 'subtitle', value: 's'},
        {kind: 'node_arr_push', path: ['doc', 'items'], value: 2},
        {kind: 'node_str_ins', path: ['doc', 'title'], pos: 1, text: 'Z'},
        {kind: 'node_replace', path: ['doc', 'title'], value: 'aZb!'},
        {kind: 'node_remove', path: ['doc', 'subtitle']},
      ],
    },
    {
      initial: {doc: {title: 'x', items: [1, 2], meta: {v: 1}}},
      ops: [
        {kind: 'node_add', path: ['doc', 'items', 1], value: 9},
        {kind: 'node_remove', path: ['doc', 'items', 0]},
        {kind: 'node_obj_put', path: ['doc'], key: 'ok', value: true},
        {kind: 'read', path: ['doc', 'items']},
        {kind: 'node_replace', path: ['doc', 'title'], value: 'xy'},
      ],
    },
    {
      initial: {doc: {title: 'hello', items: [3], tags: ['a']}},
      ops: [
        {kind: 'node_str_ins', path: ['doc', 'title'], pos: 5, text: '!'},
        {kind: 'node_arr_push', path: ['doc', 'items'], value: 4},
        {kind: 'node_obj_put', path: ['doc'], key: 'score', value: 1},
        {kind: 'node_remove', path: ['doc', 'score']},
      ],
    },
    {
      initial: {doc: {title: 'm', items: [0], active: true}},
      ops: [
        {kind: 'read', path: ['doc']},
        {kind: 'node_obj_put', path: ['doc'], key: 'active', value: false},
        {kind: 'node_add', path: ['doc', 'items', 1], value: 7},
        {kind: 'node_replace', path: ['doc', 'title'], value: 'mm'},
      ],
    },
  ];

  for (const c of deterministic) {
    fixtures.push(
      buildModelApiProxyFanoutFixture(
        `model_api_proxy_fanout_workflow_${String(idx).padStart(2, '0')}_det_v1`,
        79700 + idx,
        c.initial,
        c.ops,
      ),
    );
    idx++;
  }

  const rng = mulberry32(0x50524f58);
  for (let i = 0; i < 36; i++) {
    const initial = {
      doc: {
        title: randString(rng, 1, 8),
        items: [randInt(rng, 9), randInt(rng, 9)],
        flag: randInt(rng, 2) === 0,
      },
    };
    const ops = [
      {kind: 'read', path: ['doc', 'title']},
      {kind: 'node_obj_put', path: ['doc'], key: `k${i}`, value: randScalar(rng)},
      {kind: 'node_arr_push', path: ['doc', 'items'], value: randInt(rng, 100)},
      {kind: 'node_str_ins', path: ['doc', 'title'], pos: randInt(rng, 3), text: 'x'},
      {kind: 'node_add', path: ['doc', 'items', 1], value: randInt(rng, 100)},
      {kind: 'node_replace', path: ['doc', 'title'], value: randString(rng, 1, 10)},
      {kind: 'node_remove', path: ['doc', `k${i}`]},
    ];
    fixtures.push(
      buildModelApiProxyFanoutFixture(
        `model_api_proxy_fanout_workflow_${String(idx).padStart(2, '0')}_rnd_v1`,
        79700 + idx,
        initial,
        ops,
      ),
    );
    idx++;
  }

  return fixtures;
}

function buildCodecIndexedBinaryFixture(name, sid, data) {
  const model = mkModel(cloneJson(data), sid);
  const modelBinary = model.toBinary();
  const encoder = new IndexedBinaryEncoder();
  const encodedFields = encoder.encode(model);
  const fieldsHex = bytesRecordToHexRecord(encodedFields);
  const decoded = new IndexedBinaryDecoder().decode(hexRecordToBytesRecord(fieldsHex));
  const reEncodedFields = new IndexedBinaryEncoder().encode(decoded);

  return baseFixture(name, 'codec_indexed_binary_parity', {
    sid,
    model_binary_hex: hex(modelBinary),
  }, {
    fields_hex: fieldsHex,
    fields_roundtrip_hex: bytesRecordToHexRecord(reEncodedFields),
    view_json: normalizeView(decoded.view()),
    model_binary_hex: hex(decoded.toBinary()),
  });
}

function allCodecIndexedBinaryFixtures() {
  const fixtures = [];
  const cases = [
    {sid: 81001, data: 1},
    {sid: 81002, data: 'x'},
    {sid: 81003, data: {a: 1}},
    {sid: 81004, data: {a: 1, b: 'x'}},
    {sid: 81005, data: {a: {b: {c: 1}}}},
    {sid: 81006, data: {arr: [1, 2, 3]}},
    {sid: 81007, data: {arr: [{id: 1}, {id: 2}]}},
    {sid: 81008, data: {txt: 'hello'}},
    {sid: 81009, data: {bin: [1, 2, 3]}},
    {sid: 81010, data: {n: null}},
  ];
  let idx = 1;
  for (const c of cases) {
    fixtures.push(
      buildCodecIndexedBinaryFixture(
        `codec_indexed_binary_parity_${String(idx).padStart(2, '0')}_det_v1`,
        c.sid,
        c.data,
      ),
    );
    idx++;
  }

  const rng = mulberry32(0x1a2b3c4d);
  for (let i = 0; i < 30; i++) {
    fixtures.push(
      buildCodecIndexedBinaryFixture(
        `codec_indexed_binary_parity_${String(idx).padStart(2, '0')}_rnd_v1`,
        81100 + i,
        randJson(rng, 4),
      ),
    );
    idx++;
  }
  return fixtures;
}

function buildCodecSidecarBinaryFixture(name, sid, data) {
  const model = mkModel(cloneJson(data), sid);
  const modelBinary = model.toBinary();
  const [view, meta] = new SidecarBinaryEncoder().encode(model);
  const decodedView = new CborDecoder().read(view);
  const decoded = new SidecarBinaryDecoder().decode(decodedView, meta);
  const [view2, meta2] = new SidecarBinaryEncoder().encode(decoded);

  return baseFixture(name, 'codec_sidecar_binary_parity', {
    sid,
    model_binary_hex: hex(modelBinary),
  }, {
    view_binary_hex: hex(view),
    meta_binary_hex: hex(meta),
    view_roundtrip_binary_hex: hex(view2),
    meta_roundtrip_binary_hex: hex(meta2),
    view_json: normalizeView(decoded.view()),
    model_binary_hex: hex(decoded.toBinary()),
  });
}

function allCodecSidecarBinaryFixtures() {
  const fixtures = [];
  const cases = [
    {sid: 81201, data: 1},
    {sid: 81202, data: 'hello'},
    {sid: 81203, data: {a: 1}},
    {sid: 81204, data: {a: 1, b: 'x'}},
    {sid: 81205, data: {arr: [1, 2, 3]}},
    {sid: 81206, data: {doc: {title: 't', tags: ['x']}}},
    {sid: 81207, data: {txt: 'The quick brown fox'}},
    {sid: 81208, data: {nested: {a: {b: {c: 2}}}}},
    {sid: 81209, data: {vecish: [null, true, 1, 'x']}},
    {sid: 81210, data: null},
  ];
  let idx = 1;
  for (const c of cases) {
    fixtures.push(
      buildCodecSidecarBinaryFixture(
        `codec_sidecar_binary_parity_${String(idx).padStart(2, '0')}_det_v1`,
        c.sid,
        c.data,
      ),
    );
    idx++;
  }

  const rng = mulberry32(0x55667788);
  for (let i = 0; i < 30; i++) {
    fixtures.push(
      buildCodecSidecarBinaryFixture(
        `codec_sidecar_binary_parity_${String(idx).padStart(2, '0')}_rnd_v1`,
        81300 + i,
        randJson(rng, 4),
      ),
    );
    idx++;
  }
  return fixtures;
}

function buildPatchClockCodecFixture(name, sid, data) {
  const model = mkModel(cloneJson(data), sid);
  const table = ClockTable.from(model.clock);
  const writer = new CrdtWriter();
  table.write(writer);
  const tableBytes = writer.flush();
  const decodedTable = ClockTable.decode(new CrdtReader(tableBytes));

  const ids = [];
  model.index.forEach(({v: node}) => ids.push(node.id));
  ids.sort((a, b) => (a.time === b.time ? a.sid - b.sid : a.time - b.time));
  const pick = ids.slice(0, Math.min(4, ids.length));

  const clockEncoder = new ClockEncoder();
  clockEncoder.reset(model.clock);
  const relative = pick.map((id) => {
    const rel = clockEncoder.append(id);
    const decoder = new ClockDecoder(decodedTable.byIdx[0].sid, decodedTable.byIdx[0].time);
    for (let i = 1; i < decodedTable.byIdx.length; i++) {
      const c = decodedTable.byIdx[i];
      decoder.pushTuple(c.sid, c.time);
    }
    const decodedId = decoder.decodeId(rel.sessionIndex, rel.timeDiff);
    return {
      id: [id.sid, id.time],
      session_index: rel.sessionIndex,
      time_diff: rel.timeDiff,
      decoded_id: [decodedId.sid, decodedId.time],
    };
  });

  return baseFixture(name, 'patch_clock_codec_parity', {
    sid,
    model_binary_hex: hex(model.toBinary()),
  }, {
    clock_table_binary_hex: hex(tableBytes),
    clock_table: decodedTable.byIdx.map((c) => [c.sid, c.time]),
    relative_ids: relative,
  });
}

function allPatchClockCodecFixtures() {
  const fixtures = [];
  const baseCases = [
    {sid: 81401, data: {a: 1}},
    {sid: 81402, data: {a: {b: 1}}},
    {sid: 81403, data: {arr: [1, 2, 3]}},
    {sid: 81404, data: {txt: 'abc'}},
    {sid: 81405, data: {obj: {x: 1, y: 2}}},
  ];
  let idx = 1;
  for (const c of baseCases) {
    fixtures.push(
      buildPatchClockCodecFixture(
        `patch_clock_codec_parity_${String(idx).padStart(2, '0')}_det_v1`,
        c.sid,
        c.data,
      ),
    );
    idx++;
  }
  const rng = mulberry32(0x99aabbcc);
  for (let i = 0; i < 20; i++) {
    fixtures.push(
      buildPatchClockCodecFixture(
        `patch_clock_codec_parity_${String(idx).padStart(2, '0')}_rnd_v1`,
        81500 + i,
        randJson(rng, 4),
      ),
    );
    idx++;
  }
  return fixtures;
}

function main() {
  ensureDir(OUT_DIR);
  for (const file of fs.readdirSync(OUT_DIR)) {
    if (file.endsWith('.json')) fs.unlinkSync(path.join(OUT_DIR, file));
  }

  const fixtures = [
    ...allDiffFixtures(),
    ...allDecodeErrorFixtures(),
    ...allCanonicalEncodeFixtures(),
    ...allModelFixtures(),
    ...allModelDecodeErrorFixtures(),
    ...allModelCanonicalEncodeFixtures(),
    ...allModelApplyReplayFixtures(),
    ...allModelDiffParityFixtures(),
    ...allModelDiffDstKeysFixtures(),
    ...allLessdbModelManagerFixtures(),
    ...allModelApiWorkflowFixtures(),
    ...allModelApiProxyFanoutFixtures(),
    ...allModelLifecycleFixtures(),
    ...allCodecIndexedBinaryFixtures(),
    ...allCodecSidecarBinaryFixtures(),
    ...allPatchClockCodecFixtures(),
  ];

  for (const fixture of fixtures) {
    writeFixture(fixture.name, fixture);
  }

  const manifest = {
    fixture_version: FIXTURE_VERSION,
    upstream_package: 'json-joy',
    upstream_version: UPSTREAM_VERSION,
    fixture_count: fixtures.length,
    fixtures: fixtures.map((f) => ({name: f.name, scenario: f.scenario, file: `${f.name}.json`}))
  };

  fs.writeFileSync(path.join(OUT_DIR, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n', 'utf8');

  console.log(`wrote ${fixtures.length} fixtures to ${OUT_DIR}`);
}

main();
