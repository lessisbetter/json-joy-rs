use sonic_forest::radix::Slice;

#[test]
fn radix_slice_create_matrix() {
    let data = vec![1_u8, 2, 3, 4, 5];
    let slice = Slice::from_uint8_array(data.clone());

    assert_eq!(slice.length, 5);
    assert_eq!(slice.start, 0);
    assert_eq!(slice.to_uint8_array(), data);
}

#[test]
fn radix_slice_at_matrix() {
    let slice = Slice::from_uint8_array(vec![10_u8, 20, 30, 40, 50]);
    assert_eq!(slice.at(0), 10);
    assert_eq!(slice.at(2), 30);
    assert_eq!(slice.at(4), 50);
}

#[test]
fn radix_slice_bounds_matrix() {
    let slice = Slice::from_uint8_array(vec![1_u8, 2, 3]);

    let neg = std::panic::catch_unwind(|| {
        let _ = slice.at(usize::MAX);
    });
    assert!(neg.is_err());

    let high = std::panic::catch_unwind(|| {
        let _ = slice.at(3);
    });
    assert!(high.is_err());
}

#[test]
fn radix_slice_substring_matrix() {
    let slice = Slice::from_uint8_array(vec![1_u8, 2, 3, 4, 5]);

    let sub = slice.substring(1, Some(3));
    assert_eq!(sub.length, 3);
    assert_eq!(sub.at(0), 2);
    assert_eq!(sub.at(1), 3);
    assert_eq!(sub.at(2), 4);

    let tail = slice.substring(2, None);
    assert_eq!(tail.length, 3);
    assert_eq!(tail.at(0), 3);
    assert_eq!(tail.at(1), 4);
    assert_eq!(tail.at(2), 5);
}

#[test]
fn radix_slice_equals_compare_matrix() {
    let slice1 = Slice::from_uint8_array(vec![1_u8, 2, 3]);
    let slice2 = Slice::from_uint8_array(vec![1_u8, 2, 3]);
    let slice3 = Slice::from_uint8_array(vec![1_u8, 2, 4]);
    let slice4 = Slice::from_uint8_array(vec![1_u8, 2]);

    assert!(slice1.equals(&slice2));
    assert!(!slice1.equals(&slice3));

    assert_eq!(slice1.compare(&slice2), 0);
    assert!(slice1.compare(&slice3) < 0);
    assert!(slice3.compare(&slice1) > 0);
    assert!(slice1.compare(&slice4) > 0);
    assert!(slice4.compare(&slice1) < 0);
}

#[test]
fn radix_slice_copy_and_prefix_matrix() {
    let slice = Slice::from_uint8_array(vec![1_u8, 2, 3, 4, 5]);
    let sub = slice.substring(1, Some(3));
    let sub_array = sub.to_uint8_array();
    assert_eq!(sub_array, vec![2_u8, 3, 4]);

    let same1 = Slice::from_uint8_array(vec![1_u8, 2, 3]);
    let same2 = Slice::from_uint8_array(vec![1_u8, 2, 3]);
    assert_eq!(same1.get_common_prefix_length(&same2), 3);

    let diff1 = Slice::from_uint8_array(vec![1_u8, 2, 3, 4]);
    let diff2 = Slice::from_uint8_array(vec![1_u8, 2, 5, 6]);
    assert_eq!(diff1.get_common_prefix_length(&diff2), 2);

    let short = Slice::from_uint8_array(vec![1_u8, 2]);
    assert_eq!(same1.get_common_prefix_length(&short), 2);

    let none = Slice::from_uint8_array(vec![4_u8, 5, 6]);
    assert_eq!(same1.get_common_prefix_length(&none), 0);

    let empty = Slice::from_uint8_array(vec![]);
    assert_eq!(empty.get_common_prefix_length(&same1), 0);
}
