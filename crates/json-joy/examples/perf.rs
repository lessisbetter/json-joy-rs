//! Native Rust performance benchmark mirroring `bench/bench.mjs`.
//!
//! Run:  cargo run --example perf --release -p json-joy

use std::time::Instant;

use json_joy::json_crdt::codec::structural::binary as sbin;
use json_joy::json_crdt::model::Model;
use json_joy::json_crdt::ORIGIN;
use json_joy::json_crdt_patch::clock::Ts;
use json_joy::json_crdt_patch::patch::Patch;
use json_joy::json_crdt_patch::patch_builder::PatchBuilder;
use json_joy_json_pack::PackValue;

// ── harness ───────────────────────────────────────────────────────────────────

fn bench<F: FnMut()>(n: u32, mut f: F) -> u64 {
    let warmup = std::cmp::max(50, n / 10);
    for _ in 0..warmup {
        f();
    }
    let start = Instant::now();
    for _ in 0..n {
        f();
    }
    let elapsed = start.elapsed();
    (n as f64 / elapsed.as_secs_f64()) as u64
}

fn fmt(n: u64) -> String {
    // comma-grouped number
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn row(label: &str, ops: u64) {
    println!("  {:<20}  {:>16} op/s", label, fmt(ops));
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    println!("\n  json-joy  Rust native (no WASM overhead)\n");
    println!("  {:<20}  {:>16}", "operation", "ops/sec");
    println!("  {}", "-".repeat(42));

    let mut sid: u64 = 65_536;
    let mut next_sid = || {
        let s = sid;
        sid += 1;
        s
    };

    // ── 1. model_create ───────────────────────────────────────────────────────
    row(
        "model_create",
        bench(50_000, || {
            let _ = Model::new(next_sid());
        }),
    );

    // ── 2. set_flush  { x:1, y:2, active:false } ─────────────────────────────
    row(
        "set_flush",
        bench(10_000, || {
            let mut m = Model::new(next_sid());
            let mut b = PatchBuilder::new(m.clock.sid, m.clock.time);
            let obj_id = b.obj();
            let x = b.con_val(PackValue::Integer(1));
            let y = b.con_val(PackValue::Integer(2));
            let active = b.con_val(PackValue::Bool(false));
            b.ins_obj(
                obj_id,
                vec![
                    ("x".to_string(), x),
                    ("y".to_string(), y),
                    ("active".to_string(), active),
                ],
            );
            b.root(obj_id);
            let p = b.flush();
            m.apply_patch(&p);
            let _ = p.to_binary();
        }),
    );

    // ── 3. str_ins ×100 ───────────────────────────────────────────────────────
    {
        let chars: Vec<String> = "abcdefghijklmnopqrstuvwxyz0123456789"
            .chars()
            .map(|c| c.to_string())
            .collect();

        row(
            "str_ins×100",
            bench(500, || {
                let mut m = Model::new(next_sid());
                // api.set('') — create empty StrNode at root
                let mut b = PatchBuilder::new(m.clock.sid, m.clock.time);
                let str_id = b.str_node();
                b.root(str_id);
                m.apply_patch(&b.flush());

                // 100 sequential char insertions, each its own patch
                let mut last_ts = str_id;
                let mut last_patch = None;
                for i in 0..100usize {
                    let after = if i == 0 { str_id } else { last_ts };
                    let mut b = PatchBuilder::new(m.clock.sid, m.clock.time);
                    last_ts = b.ins_str(str_id, after, chars[i % chars.len()].clone());
                    let p = b.flush();
                    m.apply_patch(&p);
                    last_patch = Some(p);
                }
                // flush = serialize the last patch
                let _ = last_patch.unwrap().to_binary();
            }),
        );
    }

    // ── 4. binary_rt  — decode only (mirrors JS which pre-encodes once) ───────
    {
        let mut m = Model::new(next_sid());
        let patch = {
            let mut b = PatchBuilder::new(m.clock.sid, m.clock.time);
            let obj_id = b.obj();
            let name_str = b.str_node();
            b.ins_str(name_str, name_str, "test".to_string());
            let items_arr = b.arr();
            let item_ids = vec![
                b.con_val(PackValue::Integer(1)),
                b.con_val(PackValue::Integer(2)),
                b.con_val(PackValue::Integer(3)),
            ];
            b.ins_arr(items_arr, items_arr, item_ids);
            let nested = b.obj();
            let x_con = b.con_val(PackValue::Integer(0));
            b.ins_obj(nested, vec![("x".to_string(), x_con)]);
            b.ins_obj(
                obj_id,
                vec![
                    ("name".to_string(), name_str),
                    ("items".to_string(), items_arr),
                    ("nested".to_string(), nested),
                ],
            );
            b.root(obj_id);
            b.flush()
        };
        m.apply_patch(&patch);
        let bin = sbin::encode(&m);

        row(
            "binary_rt",
            bench(20_000, || {
                let _ = sbin::decode(&bin).unwrap();
            }),
        );
    }

    // ── 5. apply_patch ────────────────────────────────────────────────────────
    {
        let mut sender = Model::new(next_sid());
        let patch = {
            let mut b = PatchBuilder::new(sender.clock.sid, sender.clock.time);
            let obj_id = b.obj();
            let title_str = b.str_node();
            b.ins_str(title_str, title_str, "Hello world".to_string());
            let count_con = b.con_val(PackValue::Integer(42));
            let tags_arr = b.arr();
            let tag_ids = vec![
                b.con_val(PackValue::Str("a".to_string())),
                b.con_val(PackValue::Str("b".to_string())),
                b.con_val(PackValue::Str("c".to_string())),
            ];
            b.ins_arr(tags_arr, tags_arr, tag_ids);
            b.ins_obj(
                obj_id,
                vec![
                    ("title".to_string(), title_str),
                    ("count".to_string(), count_con),
                    ("tags".to_string(), tags_arr),
                ],
            );
            b.root(obj_id);
            b.flush()
        };
        sender.apply_patch(&patch);
        let patch_bytes = patch.to_binary();

        row(
            "apply_patch",
            bench(10_000, || {
                let mut m = Model::new(next_sid());
                let p = Patch::from_binary(&patch_bytes).unwrap();
                m.apply_patch(&p);
            }),
        );
    }

    // ── 6. obj_set  { x:1, y:2, label:'point', active:true } ─────────────────
    row(
        "obj_set",
        bench(20_000, || {
            let mut m = Model::new(next_sid());
            // api.set({})
            let mut b = PatchBuilder::new(m.clock.sid, m.clock.time);
            let obj_id = b.obj();
            b.root(obj_id);
            m.apply_patch(&b.flush());
            // obj([]).set({...})
            let mut b = PatchBuilder::new(m.clock.sid, m.clock.time);
            let x = b.con_val(PackValue::Integer(1));
            let y = b.con_val(PackValue::Integer(2));
            let label = b.con_val(PackValue::Str("point".to_string()));
            let active = b.con_val(PackValue::Bool(true));
            b.ins_obj(
                obj_id,
                vec![
                    ("x".to_string(), x),
                    ("y".to_string(), y),
                    ("label".to_string(), label),
                    ("active".to_string(), active),
                ],
            );
            let p = b.flush();
            m.apply_patch(&p);
            let _ = p.to_binary();
        }),
    );

    // ── 7. arr_ins ×20 ────────────────────────────────────────────────────────
    row(
        "arr_ins×20",
        bench(5_000, || {
            let mut m = Model::new(next_sid());
            // api.set([])
            let mut b = PatchBuilder::new(m.clock.sid, m.clock.time);
            let arr_id = b.arr();
            b.root(arr_id);
            m.apply_patch(&b.flush());
            // arr([]).ins(0, [0..19])
            let mut b = PatchBuilder::new(m.clock.sid, m.clock.time);
            let ids: Vec<Ts> = (0i64..20)
                .map(|v| b.con_val(PackValue::Integer(v)))
                .collect();
            b.ins_arr(arr_id, ORIGIN, ids);
            let p = b.flush();
            m.apply_patch(&p);
            let _ = p.to_binary();
        }),
    );

    // ── 8. view  — steady-state { name:'Alice', age:30, items:[1,2,3], active:true }
    {
        let mut m = Model::new(next_sid());
        let patch = {
            let mut b = PatchBuilder::new(m.clock.sid, m.clock.time);
            let obj_id = b.obj();
            let name_str = b.str_node();
            b.ins_str(name_str, name_str, "Alice".to_string());
            let age_con = b.con_val(PackValue::Integer(30));
            let items_arr = b.arr();
            let item_ids = vec![
                b.con_val(PackValue::Integer(1)),
                b.con_val(PackValue::Integer(2)),
                b.con_val(PackValue::Integer(3)),
            ];
            b.ins_arr(items_arr, items_arr, item_ids);
            let active_con = b.con_val(PackValue::Bool(true));
            b.ins_obj(
                obj_id,
                vec![
                    ("name".to_string(), name_str),
                    ("age".to_string(), age_con),
                    ("items".to_string(), items_arr),
                    ("active".to_string(), active_con),
                ],
            );
            b.root(obj_id);
            b.flush()
        };
        m.apply_patch(&patch);

        row(
            "view",
            bench(200_000, || {
                let _ = m.view();
            }),
        );
    }

    println!();
}
