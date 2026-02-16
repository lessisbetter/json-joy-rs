use super::str;

pub fn to_str(buf: &[u8]) -> String {
    let mut s = String::with_capacity(buf.len());
    for b in buf {
        s.push(char::from(*b));
    }
    s
}

pub fn to_bin(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u32 as u8).collect()
}

pub fn diff(src: &[u8], dst: &[u8]) -> str::Patch {
    str::diff(&to_str(src), &to_str(dst))
}

pub fn apply<FIns, FDel>(
    patch: &str::Patch,
    src_len: usize,
    mut on_insert: FIns,
    mut on_delete: FDel,
) where
    FIns: FnMut(usize, Vec<u8>),
    FDel: FnMut(usize, usize),
{
    str::apply(
        patch,
        src_len,
        |pos, s| on_insert(pos, to_bin(s)),
        |pos, len, _| on_delete(pos, len),
    );
}

pub fn src(patch: &str::Patch) -> Vec<u8> {
    to_bin(&str::src(patch))
}

pub fn dst(patch: &str::Patch) -> Vec<u8> {
    to_bin(&str::dst(patch))
}
