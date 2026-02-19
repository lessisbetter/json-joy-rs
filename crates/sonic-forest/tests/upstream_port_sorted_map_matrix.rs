use std::collections::BTreeMap;

use sonic_forest::SortedMap;

#[test]
fn sorted_map_numbers_from_0_to_100_matrix() {
    let mut map = SortedMap::<i32, i32>::new();
    for i in 0..=100 {
        map.set_element(i, i, None);
        assert_eq!(map.size(), (i + 1) as usize);
    }
    for i in 0..=100 {
        map.erase_element_by_key(&i);
        assert_eq!(map.size(), (100 - i) as usize);
    }
}

#[test]
fn sorted_map_numbers_both_directions_from_50_matrix() {
    let mut map = SortedMap::<i32, i32>::new();
    for i in 1..=100 {
        map.set_element(50 + i, 50 + i, None);
        map.set_element(50 - i, 50 - i, None);
        assert_eq!(map.size(), ((i - 1) * 2 + 2) as usize);
    }
    for i in 1..=100 {
        map.erase_element_by_key(&(50 - i));
        map.erase_element_by_key(&(50 + i));
    }
    assert_eq!(map.size(), 0);
}

fn next_pseudo(seed: &mut u64) -> i32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    ((*seed >> 33) % 101) as i32
}

#[test]
fn sorted_map_random_numbers_from_0_to_100_matrix() {
    let mut map = SortedMap::<i32, i32>::new();
    let mut seed = 0x5EED_u64;

    for _ in 0..=1000 {
        let num = next_pseudo(&mut seed);
        let found = map.get_element_by_key(&num).is_some();
        if !found {
            map.set_element(num, num, None);
        }
    }

    let size1 = map.size();
    assert!(size1 > 4);

    for _ in 0..=400 {
        let num = next_pseudo(&mut seed);
        map.erase_element_by_key(&num);
    }

    let size2 = map.size();
    assert!(size2 < size1);
}

#[test]
fn sorted_map_bounds_and_iterators_matrix() {
    let mut map = SortedMap::<i32, i32>::new();
    map.set_element(1, 10, None);
    map.set_element(3, 30, None);
    map.set_element(5, 50, None);

    let lb0 = map.lower_bound(&0);
    assert_eq!(lb0.index(), 0);
    assert!(lb0.is_accessible());

    let lb2 = map.lower_bound(&2);
    assert_eq!(lb2.index(), 1);
    assert!(lb2.is_accessible());

    let lb6 = map.lower_bound(&6);
    assert!(!lb6.is_accessible());

    let ub3 = map.upper_bound(&3);
    assert_eq!(ub3.index(), 2);
    assert!(ub3.is_accessible());

    let rlb4 = map.reverse_lower_bound(&4);
    assert_eq!(rlb4.index(), 1);
    assert!(rlb4.is_accessible());

    let rub3 = map.reverse_upper_bound(&3);
    assert_eq!(rub3.index(), 0);
    assert!(rub3.is_accessible());
}

#[test]
fn sorted_map_update_key_by_iterator_matrix() {
    let mut map = SortedMap::<i32, i32>::new();
    map.set_element(1, 10, None);
    map.set_element(3, 30, None);
    map.set_element(5, 50, None);

    let mid = map.lower_bound(&3);
    assert!(map.update_key_by_iterator(&mid, 4));
    assert_eq!(map.get_element_by_key(&3), None);
    assert_eq!(map.get_element_by_key(&4), Some(&30));

    let mid2 = map.lower_bound(&4);
    assert!(!map.update_key_by_iterator(&mid2, 6));
    assert_eq!(map.get_element_by_key(&4), Some(&30));
}

#[test]
fn sorted_map_erase_element_by_iterator_matrix() {
    let mut map = SortedMap::<i32, i32>::new();
    map.set_element(1, 1, None);
    map.set_element(2, 2, None);
    map.set_element(3, 3, None);

    let it = map.lower_bound(&2);
    let next_it = map.erase_element_by_iterator(it);
    assert_eq!(map.size(), 2);
    assert_eq!(map.get_element_by_key(&2), None);
    assert!(next_it.is_accessible());
    assert_eq!(next_it.index(), 1);
}

#[test]
fn sorted_map_erase_element_by_reverse_iterator_matrix() {
    let mut map = SortedMap::<i32, i32>::new();
    map.set_element(1, 1, None);
    map.set_element(2, 2, None);
    map.set_element(3, 3, None);

    let it = map.r_begin();
    assert!(it.is_accessible());
    assert_eq!(it.index(), 2);

    let next_it = map.erase_element_by_iterator(it);
    assert_eq!(map.size(), 2);
    assert_eq!(map.get_element_by_key(&3), None);
    assert!(next_it.is_accessible());
    assert_eq!(next_it.index(), 1);
}

#[test]
fn sorted_map_differential_against_btreemap_matrix() {
    let mut map = SortedMap::<i32, i32>::new();
    let mut shadow = BTreeMap::<i32, i32>::new();
    let mut seed: u64 = 0xD00D_F00D_u64;

    let mut rand_u64 = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        seed
    };

    for step in 0..2000 {
        let key = ((rand_u64() % 401) as i32) - 200;
        let roll = (rand_u64() % 100) as i32;

        if roll < 55 {
            let value = ((rand_u64() % 10_000) as i32) - 5000;
            map.set_element(key, value, None);
            shadow.insert(key, value);
        } else if roll < 85 {
            let deleted = map.erase_element_by_key(&key);
            let shadow_deleted = shadow.remove(&key).is_some();
            assert_eq!(deleted, shadow_deleted);
        } else {
            assert_eq!(
                map.get_element_by_key(&key).copied(),
                shadow.get(&key).copied()
            );
        }

        if step % 40 == 0 {
            assert_eq!(map.size(), shadow.len());

            let front = map.front().map(|(k, v)| (*k, *v));
            let shadow_front = shadow.first_key_value().map(|(k, v)| (*k, *v));
            assert_eq!(front, shadow_front);

            let back = map.back().map(|(k, v)| (*k, *v));
            let shadow_back = shadow.last_key_value().map(|(k, v)| (*k, *v));
            assert_eq!(back, shadow_back);

            for (idx, (k, v)) in shadow.iter().enumerate() {
                assert_eq!(map.get_element_by_key(k), Some(v));

                let lb = map.lower_bound(k);
                assert!(lb.is_accessible());
                assert_eq!(lb.index(), idx);

                let ub = map.upper_bound(k);
                if idx + 1 < shadow.len() {
                    assert!(ub.is_accessible());
                    assert_eq!(ub.index(), idx + 1);
                } else {
                    assert!(!ub.is_accessible());
                }

                let rlb = map.reverse_lower_bound(k);
                assert!(rlb.is_accessible());
                assert_eq!(rlb.index(), idx);

                let rub = map.reverse_upper_bound(k);
                if idx > 0 {
                    assert!(rub.is_accessible());
                    assert_eq!(rub.index(), idx - 1);
                } else {
                    assert!(!rub.is_accessible());
                }
            }

            if let Some((min_k, _)) = shadow.first_key_value() {
                let below_min = *min_k - 1;
                let lb = map.lower_bound(&below_min);
                assert!(lb.is_accessible());
                assert_eq!(lb.index(), 0);
            } else {
                assert!(!map.begin().is_accessible());
                assert!(!map.r_begin().is_accessible());
            }

            if let Some((max_k, _)) = shadow.last_key_value() {
                let above_max = *max_k + 1;
                assert!(!map.lower_bound(&above_max).is_accessible());
                assert!(map.reverse_lower_bound(&above_max).is_accessible());
            }
        }
    }
}
