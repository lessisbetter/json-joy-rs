use json_joy_core::codec_indexed_binary::{
    decode_fields_to_model_binary, encode_model_binary_to_fields,
};
use json_joy_core::codec_sidecar_binary::{
    decode_sidecar_to_model_binary, encode_model_binary_to_sidecar,
};
use json_joy_core::less_db_compat::{create_model, model_to_binary};
use json_joy_core::model::Model;
use serde_json::Value;

#[test]
fn property_codec_roundtrip_invariants_hold_for_seeded_models() {
    for (i, seed) in seeds().iter().enumerate() {
        let sid = 92000 + i as u64;
        let value = random_json(*seed, 4);
        let model = create_model(&value, sid).expect("create_model must succeed");
        let base_binary = model_to_binary(&model);
        let base_view = Model::from_binary(&base_binary)
            .expect("base binary must decode")
            .view()
            .clone();

        let fields =
            encode_model_binary_to_fields(&base_binary).expect("indexed encode must succeed");
        let indexed_binary =
            decode_fields_to_model_binary(&fields).expect("indexed decode must succeed");
        let indexed_view = Model::from_binary(&indexed_binary)
            .expect("indexed binary must decode")
            .view()
            .clone();
        assert_eq!(
            indexed_view, base_view,
            "indexed view invariant mismatch seed={seed}"
        );

        let (side_view, side_meta) =
            encode_model_binary_to_sidecar(&base_binary).expect("sidecar encode must succeed");
        let sidecar_binary = decode_sidecar_to_model_binary(&side_view, &side_meta)
            .expect("sidecar decode must succeed");
        let sidecar_view = Model::from_binary(&sidecar_binary)
            .expect("sidecar binary must decode")
            .view()
            .clone();
        assert_eq!(
            sidecar_view, base_view,
            "sidecar view invariant mismatch seed={seed}"
        );
    }
}

fn seeds() -> [u64; 20] {
    [
        0x5eed_c0de_u64,
        0x0000_0000_0000_0001_u64,
        0x0000_0000_0000_00ff_u64,
        0x0000_0000_00c0_ffee_u64,
        0x0123_4567_89ab_cdef_u64,
        0x0000_0000_0000_1001_u64,
        0x0000_0000_0000_2002_u64,
        0x0000_0000_0000_3003_u64,
        0x0000_0000_0000_4004_u64,
        0x0000_0000_0000_5005_u64,
        0x1111_2222_3333_4444_u64,
        0x2222_3333_4444_5555_u64,
        0x3333_4444_5555_6666_u64,
        0x4444_5555_6666_7777_u64,
        0x5555_6666_7777_8888_u64,
        0x89ab_cdef_0123_4567_u64,
        0xfedc_ba98_7654_3210_u64,
        0x1357_9bdf_2468_ace0_u64,
        0x0f0f_f0f0_55aa_aa55_u64,
        0xa5a5_5a5a_dead_beef_u64,
    ]
}

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn range(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }
}

fn random_scalar(rng: &mut Lcg) -> Value {
    match rng.range(5) {
        0 => Value::Null,
        1 => Value::Bool(rng.range(2) == 1),
        2 => Value::Number(serde_json::Number::from((rng.range(50) as i64) - 10)),
        3 => Value::String(format!("s{}", rng.range(100))),
        _ => Value::String("".to_string()),
    }
}

fn random_value(rng: &mut Lcg, depth: usize) -> Value {
    if depth == 0 {
        return random_scalar(rng);
    }
    match rng.range(4) {
        0 => random_scalar(rng),
        1 => {
            let len = rng.range(4) as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(random_value(rng, depth - 1));
            }
            Value::Array(arr)
        }
        _ => random_object(rng, depth - 1),
    }
}

fn random_object(rng: &mut Lcg, depth: usize) -> Value {
    let len = (1 + rng.range(4)) as usize;
    let mut map = serde_json::Map::new();
    for i in 0..len {
        map.insert(format!("k{}", i), random_value(rng, depth));
    }
    Value::Object(map)
}

fn random_json(seed: u64, depth: usize) -> Value {
    let mut rng = Lcg::new(seed);
    random_object(&mut rng, depth)
}
