use super::types::{cmp_id_time_sid, ArrAtom, BinAtom, Id, StrAtom};

fn find_insert_index_rga(slots: &[Id], reference: Id, container: Id, id: Id) -> Option<usize> {
    let mut left = if reference == container {
        if slots.is_empty() {
            return Some(0);
        }
        let first = slots[0];
        if cmp_id_time_sid(first, id).is_lt() {
            return Some(0);
        }
        if first == id {
            return None;
        }
        0usize
    } else {
        slots.iter().position(|slot| *slot == reference)?
    };

    loop {
        let right = left + 1;
        if right >= slots.len() {
            break;
        }
        let right_id = slots[right];
        if right_id.time < id.time {
            break;
        }
        if right_id.time == id.time {
            if right_id.sid == id.sid {
                return None;
            }
            if right_id.sid < id.sid {
                break;
            }
        }
        left = right;
    }

    Some(left + 1)
}

pub(crate) fn find_insert_index_str(
    atoms: &[StrAtom],
    reference: Id,
    container: Id,
    id: Id,
) -> Option<usize> {
    let slots = atoms.iter().map(|a| a.slot).collect::<Vec<_>>();
    find_insert_index_rga(&slots, reference, container, id)
}

pub(crate) fn find_insert_index_bin(
    atoms: &[BinAtom],
    reference: Id,
    container: Id,
    id: Id,
) -> Option<usize> {
    let slots = atoms.iter().map(|a| a.slot).collect::<Vec<_>>();
    find_insert_index_rga(&slots, reference, container, id)
}

pub(crate) fn find_insert_index_arr(
    atoms: &[ArrAtom],
    reference: Id,
    container: Id,
    id: Id,
) -> Option<usize> {
    let slots = atoms.iter().map(|a| a.slot).collect::<Vec<_>>();
    find_insert_index_rga(&slots, reference, container, id)
}
