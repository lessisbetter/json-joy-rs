use json_joy_core::less_db_compat;
use json_joy_core::patch::Patch;
use json_joy_core::patch_log;

pub fn version() -> String {
    json_joy_core::version().to_owned()
}

pub fn generate_session_id() -> u64 {
    json_joy_core::generate_session_id()
}

pub fn is_valid_session_id(sid: u64) -> bool {
    json_joy_core::is_valid_session_id(sid)
}

pub fn compat_model_create(data_json: String, sid: u64) -> Vec<u8> {
    let value: serde_json::Value = serde_json::from_str(&data_json).expect("invalid JSON input");
    let model = less_db_compat::create_model(&value, sid).expect("compat_model_create failed");
    less_db_compat::model_to_binary(&model)
}

pub fn compat_model_from_binary(model_binary: Vec<u8>) -> Vec<u8> {
    let model = less_db_compat::model_from_binary(&model_binary).expect("compat_model_from_binary failed");
    less_db_compat::model_to_binary(&model)
}

pub fn compat_model_load(model_binary: Vec<u8>, sid: u64) -> Vec<u8> {
    let model = less_db_compat::model_load(&model_binary, sid).expect("compat_model_load failed");
    less_db_compat::model_to_binary(&model)
}

pub fn compat_model_view(model_binary: Vec<u8>) -> String {
    let model = less_db_compat::model_from_binary(&model_binary).expect("compat_model_view failed");
    serde_json::to_string(&less_db_compat::view_model(&model)).expect("serialize view json failed")
}

pub fn compat_model_diff(model_binary: Vec<u8>, sid: u64, next_json: String) -> Vec<u8> {
    let model = less_db_compat::model_load(&model_binary, sid).expect("compat_model_diff load failed");
    let next: serde_json::Value = serde_json::from_str(&next_json).expect("invalid next JSON");
    less_db_compat::diff_model(&model, &next)
        .expect("compat_model_diff failed")
        .unwrap_or_default()
}

pub fn compat_model_apply(model_binary: Vec<u8>, patch_binary: Vec<u8>) -> Vec<u8> {
    let mut model = less_db_compat::model_from_binary(&model_binary).expect("compat_model_apply load failed");
    less_db_compat::apply_patch(&mut model, &patch_binary).expect("compat_model_apply failed");
    less_db_compat::model_to_binary(&model)
}

pub fn compat_model_fork(model_binary: Vec<u8>, sid: i64) -> Vec<u8> {
    let model = less_db_compat::model_from_binary(&model_binary).expect("compat_model_fork load failed");
    let forked = if sid < 0 {
        less_db_compat::fork_model(&model, None)
    } else {
        less_db_compat::fork_model(&model, Some(sid as u64))
    }
    .expect("compat_model_fork failed");
    less_db_compat::model_to_binary(&forked)
}

pub fn compat_patch_log_serialize(patch_binaries: Vec<Vec<u8>>) -> Vec<u8> {
    let patches: Vec<Patch> = patch_binaries
        .iter()
        .map(|b| Patch::from_binary(b).expect("invalid patch binary"))
        .collect();
    patch_log::serialize_patches(&patches)
}

pub fn compat_patch_log_deserialize(log_binary: Vec<u8>) -> Vec<Vec<u8>> {
    patch_log::deserialize_patches(&log_binary)
        .expect("patch log deserialize failed")
        .into_iter()
        .map(|p| p.to_binary())
        .collect()
}

pub fn compat_patch_log_append(existing: Vec<u8>, patch_binary: Vec<u8>) -> Vec<u8> {
    let patch = Patch::from_binary(&patch_binary).expect("invalid patch binary");
    patch_log::append_patch(&existing, &patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_compat_create_diff_apply_view_roundtrip() {
        let model = compat_model_create("{\"title\":\"a\"}".to_string(), 79501);
        let patch = compat_model_diff(model.clone(), 79501, "{\"title\":\"A\"}".to_string());
        assert!(!patch.is_empty(), "diff should produce a patch");

        let applied = compat_model_apply(model, patch);
        let view = compat_model_view(applied);
        assert_eq!(view, "{\"title\":\"A\"}");
    }

    #[test]
    fn ffi_compat_patch_log_append_and_deserialize() {
        let model = compat_model_create("{\"n\":1}".to_string(), 79502);
        let patch = compat_model_diff(model, 79502, "{\"n\":2}".to_string());
        assert!(!patch.is_empty(), "diff should produce a patch");

        let log = compat_patch_log_append(Vec::new(), patch.clone());
        let patches = compat_patch_log_deserialize(log);
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0], patch);
    }
}

uniffi::include_scaffolding!("json_joy");
