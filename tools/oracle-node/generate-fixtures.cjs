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
    patch_span: decoded.span(),
    patch_id_sid: patchId ? patchId.sid : null,
    patch_id_time: patchId ? patchId.time : null,
    patch_next_time: decoded.nextTime(),
    view_after_apply_json: model.view(),
    model_binary_after_apply_hex: hex(model.toBinary())
  });
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

function main() {
  ensureDir(OUT_DIR);
  for (const file of fs.readdirSync(OUT_DIR)) {
    if (file.endsWith('.json')) fs.unlinkSync(path.join(OUT_DIR, file));
  }

  const fixtures = [...allDiffFixtures(), ...allDecodeErrorFixtures()];

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
