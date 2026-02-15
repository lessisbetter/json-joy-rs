const fs = require('node:fs');
const path = require('node:path');
const {Model} = require('json-joy/lib/json-crdt/index.js');
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
  return [
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
    {name: 'decode_error_long_random_v1', hex: 'abcd'.repeat(32)}
  ];

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
  ];
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
