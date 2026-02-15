const fs = require('node:fs');
const {Model} = require('json-joy/lib/json-crdt/index.js');
const {Patch} = require('json-joy/lib/json-crdt-patch/index.js');

function fromHex(hexStr) {
  return new Uint8Array(Buffer.from(hexStr, 'hex'));
}

function hex(bytes) {
  return Buffer.from(bytes).toString('hex');
}

function main() {
  const inputRaw = process.argv[2] ?? fs.readFileSync(0, 'utf8');
  const input = JSON.parse(inputRaw);

  const base = fromHex(input.base_model_binary_hex);
  const sid = input.sid;
  const next = input.next_view_json;

  const model = Model.load(base, sid);
  const patch = model.api.diff(next);

  if (!patch) {
    process.stdout.write(JSON.stringify({patch_present: false}));
    return;
  }

  const patchBinary = patch.toBinary();
  const decoded = Patch.fromBinary(patchBinary);
  const patchId = decoded.getId();

  process.stdout.write(
    JSON.stringify({
      patch_present: true,
      patch_binary_hex: hex(patchBinary),
      patch_op_count: decoded.ops.length,
      patch_opcodes: decoded.ops.map((op) => op.name()),
      patch_span: decoded.span(),
      patch_id_sid: patchId ? patchId.sid : null,
      patch_id_time: patchId ? patchId.time : null,
      patch_next_time: decoded.nextTime(),
    }),
  );
}

main();
