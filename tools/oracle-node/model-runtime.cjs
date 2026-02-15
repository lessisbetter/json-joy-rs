const fs = require('node:fs');
const {Model} = require('json-joy/lib/json-crdt/index.js');
const {Patch} = require('json-joy/lib/json-crdt-patch/index.js');

function fromHex(hexStr) {
  return new Uint8Array(Buffer.from(hexStr, 'hex'));
}

function hex(bytes) {
  return Buffer.from(bytes).toString('hex');
}

function normalizeView(view) {
  return view === undefined ? null : view;
}

function run(input) {
  const op = input.op;

  if (op === 'create') {
    const model = Model.create(undefined, input.sid);
    model.api.set(input.data_json);
    model.api.flush();
    return {
      model_binary_hex: hex(model.toBinary()),
      view_json: normalizeView(model.view()),
      sid: model.clock.sid,
    };
  }

  if (op === 'from_binary') {
    const model = Model.fromBinary(fromHex(input.model_binary_hex));
    return {
      model_binary_hex: hex(model.toBinary()),
      view_json: normalizeView(model.view()),
      sid: model.clock.sid,
    };
  }

  if (op === 'load') {
    const model = Model.load(fromHex(input.model_binary_hex), input.sid);
    return {
      model_binary_hex: hex(model.toBinary()),
      view_json: normalizeView(model.view()),
      sid: model.clock.sid,
    };
  }

  if (op === 'diff') {
    const model = Model.load(fromHex(input.model_binary_hex), input.sid);
    const patch = model.api.diff(input.next_view_json);
    if (!patch) {
      return {patch_present: false};
    }
    const patchBinary = patch.toBinary();
    const decoded = Patch.fromBinary(patchBinary);
    const patchId = decoded.getId();
    return {
      patch_present: true,
      patch_binary_hex: hex(patchBinary),
      patch_op_count: decoded.ops.length,
      patch_opcodes: decoded.ops.map((op) => op.name()),
      patch_span: decoded.span(),
      patch_id_sid: patchId ? patchId.sid : null,
      patch_id_time: patchId ? patchId.time : null,
      patch_next_time: decoded.nextTime(),
    };
  }

  if (op === 'apply_patch') {
    const model = Model.fromBinary(fromHex(input.model_binary_hex));
    model.applyPatch(Patch.fromBinary(fromHex(input.patch_binary_hex)));
    return {
      model_binary_hex: hex(model.toBinary()),
      view_json: normalizeView(model.view()),
      sid: model.clock.sid,
    };
  }

  if (op === 'fork') {
    const model = Model.fromBinary(fromHex(input.model_binary_hex));
    const fork = input.sid === null || input.sid === undefined ? model.fork() : model.fork(input.sid);
    return {
      model_binary_hex: hex(fork.toBinary()),
      view_json: normalizeView(fork.view()),
      sid: fork.clock.sid,
    };
  }

  if (op === 'merge') {
    const model = Model.fromBinary(fromHex(input.model_binary_hex));
    for (const patchHex of input.patches_binary_hex) {
      model.applyPatch(Patch.fromBinary(fromHex(patchHex)));
    }
    return {
      model_binary_hex: hex(model.toBinary()),
      view_json: normalizeView(model.view()),
      sid: model.clock.sid,
    };
  }

  throw new Error(`unsupported op: ${op}`);
}

function main() {
  const inputRaw = process.argv[2] ?? fs.readFileSync(0, 'utf8');
  const input = JSON.parse(inputRaw);
  const out = run(input);
  process.stdout.write(JSON.stringify(out));
}

main();
