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
