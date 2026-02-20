use crate::red_black::util::assert_red_black_tree;

use super::llrb_tree::LlrbNode;

/// Check if a node is red (non-black).
pub fn is_red<K, V>(arena: &[LlrbNode<K, V>], node: Option<u32>) -> bool {
    node.map(|i| !arena[i as usize].b).unwrap_or(false)
}

/// Flip colors of node and its children.
pub fn color_flip<K, V>(arena: &mut [LlrbNode<K, V>], node: u32) {
    arena[node as usize].b = !arena[node as usize].b;
    if let Some(l) = arena[node as usize].l {
        arena[l as usize].b = !arena[l as usize].b;
    }
    if let Some(r) = arena[node as usize].r {
        arena[r as usize].b = !arena[r as usize].b;
    }
}

/// Rotate left with parent pointer updates.
pub fn rotate_left<K, V>(arena: &mut [LlrbNode<K, V>], node: u32) -> u32 {
    let x = arena[node as usize]
        .r
        .expect("rotate_left requires right child");
    let xl = arena[x as usize].l;
    arena[node as usize].r = xl;
    if let Some(xl) = xl {
        arena[xl as usize].p = Some(node);
    }

    arena[x as usize].l = Some(node);
    let node_p = arena[node as usize].p;
    arena[x as usize].p = node_p;
    arena[node as usize].p = Some(x);

    if let Some(p) = node_p {
        if arena[p as usize].l == Some(node) {
            arena[p as usize].l = Some(x);
        } else {
            arena[p as usize].r = Some(x);
        }
    }

    arena[x as usize].b = arena[node as usize].b;
    arena[node as usize].b = false;
    x
}

/// Rotate right with parent pointer updates.
pub fn rotate_right<K, V>(arena: &mut [LlrbNode<K, V>], node: u32) -> u32 {
    let x = arena[node as usize]
        .l
        .expect("rotate_right requires left child");
    let xr = arena[x as usize].r;
    arena[node as usize].l = xr;
    if let Some(xr) = xr {
        arena[xr as usize].p = Some(node);
    }

    arena[x as usize].r = Some(node);
    let node_p = arena[node as usize].p;
    arena[x as usize].p = node_p;
    arena[node as usize].p = Some(x);

    if let Some(p) = node_p {
        if arena[p as usize].l == Some(node) {
            arena[p as usize].l = Some(x);
        } else {
            arena[p as usize].r = Some(x);
        }
    }

    arena[x as usize].b = arena[node as usize].b;
    arena[node as usize].b = false;
    x
}

/// Move red link to the left.
pub fn move_red_left<K, V>(arena: &mut [LlrbNode<K, V>], mut node: u32) -> u32 {
    color_flip(arena, node);
    if let Some(r) = arena[node as usize].r {
        if is_red(arena, arena[r as usize].l) {
            let rotated_right = rotate_right(arena, r);
            arena[node as usize].r = Some(rotated_right);
            node = rotate_left(arena, node);
            color_flip(arena, node);
        }
    }
    node
}

/// Move red link to the right.
pub fn move_red_right<K, V>(arena: &mut [LlrbNode<K, V>], mut node: u32) -> u32 {
    color_flip(arena, node);
    if let Some(l) = arena[node as usize].l {
        if is_red(arena, arena[l as usize].l) {
            node = rotate_right(arena, node);
            color_flip(arena, node);
        }
    }
    node
}

/// Balance the LLRB tree after modifications.
pub fn balance<K, V>(arena: &mut [LlrbNode<K, V>], mut node: u32) -> u32 {
    if is_red(arena, arena[node as usize].r) {
        node = rotate_left(arena, node);
    }
    if is_red(arena, arena[node as usize].l)
        && match arena[node as usize].l {
            Some(l) => is_red(arena, arena[l as usize].l),
            None => false,
        }
    {
        node = rotate_right(arena, node);
    }
    if is_red(arena, arena[node as usize].l) && is_red(arena, arena[node as usize].r) {
        color_flip(arena, node);
    }
    node
}

/// Delete the minimum node from the subtree.
pub fn delete_min<K, V>(arena: &mut Vec<LlrbNode<K, V>>, node: u32) -> Option<u32> {
    arena[node as usize].l?;

    let mut node = node;
    let l = arena[node as usize].l.expect("left exists");
    if !is_red(arena, Some(l)) && !is_red(arena, arena[l as usize].l) {
        node = move_red_left(arena, node);
    }

    let next_left = arena[node as usize]
        .l
        .expect("left exists after move_red_left");
    arena[node as usize].l = delete_min(arena, next_left);
    if let Some(l) = arena[node as usize].l {
        arena[l as usize].p = Some(node);
    }

    Some(balance(arena, node))
}

/// Find the minimum node in the subtree.
pub fn min<K, V>(arena: &[LlrbNode<K, V>], mut node: u32) -> u32 {
    while let Some(l) = arena[node as usize].l {
        node = l;
    }
    node
}

fn swap_key_value<K, V>(arena: &mut [LlrbNode<K, V>], a: u32, b: u32) {
    let ai = a as usize;
    let bi = b as usize;
    if ai == bi {
        return;
    }

    if ai < bi {
        let (left, right) = arena.split_at_mut(bi);
        let an = &mut left[ai];
        let bn = &mut right[0];
        std::mem::swap(&mut an.k, &mut bn.k);
        std::mem::swap(&mut an.v, &mut bn.v);
    } else {
        let (left, right) = arena.split_at_mut(ai);
        let bn = &mut left[bi];
        let an = &mut right[0];
        std::mem::swap(&mut an.k, &mut bn.k);
        std::mem::swap(&mut an.v, &mut bn.v);
    }
}

/// Delete a node with the given key from the subtree.
pub fn delete_node<K, V, C>(
    arena: &mut Vec<LlrbNode<K, V>>,
    node: Option<u32>,
    key: &K,
    comparator: &C,
) -> Option<u32>
where
    C: Fn(&K, &K) -> i32,
{
    let mut node = node?;

    let cmp = comparator(key, &arena[node as usize].k);

    if cmp < 0 {
        if arena[node as usize].l.is_none() {
            return Some(node);
        }

        let l = arena[node as usize].l.expect("left exists");
        if !is_red(arena, Some(l)) && !is_red(arena, arena[l as usize].l) {
            node = move_red_left(arena, node);
        }

        let left = arena[node as usize].l;
        arena[node as usize].l = delete_node(arena, left, key, comparator);
        if let Some(l) = arena[node as usize].l {
            arena[l as usize].p = Some(node);
        }
    } else {
        if is_red(arena, arena[node as usize].l) {
            node = rotate_right(arena, node);
        }

        if comparator(key, &arena[node as usize].k) == 0 && arena[node as usize].r.is_none() {
            return None;
        }

        if arena[node as usize].r.is_none() {
            return Some(node);
        }

        let r = arena[node as usize].r.expect("right exists");
        if !is_red(arena, Some(r)) && !is_red(arena, arena[r as usize].l) {
            node = move_red_right(arena, node);
        }

        if comparator(key, &arena[node as usize].k) == 0 {
            let right = arena[node as usize].r.expect("right exists");
            let min_node = min(arena, right);
            swap_key_value(arena, node, min_node);
            let right = arena[node as usize].r.expect("right exists after key swap");
            arena[node as usize].r = delete_min(arena, right);
            if let Some(r) = arena[node as usize].r {
                arena[r as usize].p = Some(node);
            }
        } else {
            let right = arena[node as usize].r;
            arena[node as usize].r = delete_node(arena, right, key, comparator);
            if let Some(r) = arena[node as usize].r {
                arena[r as usize].p = Some(node);
            }
        }
    }

    Some(balance(arena, node))
}

pub fn assert_llrb_tree<K, V, C>(
    arena: &[LlrbNode<K, V>],
    root: Option<u32>,
    comparator: &C,
) -> Result<(), String>
where
    C: Fn(&K, &K) -> i32,
{
    assert_red_black_tree(arena, root, comparator)?;

    fn assert_left_leaning<K, V>(arena: &[LlrbNode<K, V>], node: u32) -> Result<(), String> {
        let l = arena[node as usize].l;
        let r = arena[node as usize].r;

        if let Some(r) = r {
            if !arena[r as usize].b && l.map(|i| arena[i as usize].b).unwrap_or(true) {
                return Err(
                    "Left-leaning property violated: red right child with black/null left child"
                        .to_string(),
                );
            }
        }

        if let Some(l) = l {
            assert_left_leaning(arena, l)?;
        }
        if let Some(r) = r {
            assert_left_leaning(arena, r)?;
        }
        Ok(())
    }

    if let Some(root) = root {
        assert_left_leaning(arena, root)?;
    }

    Ok(())
}
