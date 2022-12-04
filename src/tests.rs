use core::cmp::Ordering;
use crate::AltDeque;

#[test]
fn test_new() {
    let deque = AltDeque::<u64>::new();
    assert_eq!(deque.capacity(), 0);
    assert_eq!(deque.len(), 0);
    assert_eq!(deque, []);
}

#[test]
fn test_with_capacity() {
    let deque = AltDeque::<u64>::with_capacity(8);
    assert_eq!(deque.capacity(), 8);
    assert_eq!(deque.len(), 0);
    assert_eq!(deque, []);
}

#[test]
fn test_len_and_empty() {
    let mut deque = AltDeque::from([1, 2, 3]);
    assert_eq!(deque.len(), 3);
    assert!(!deque.is_empty());
    assert_eq!(deque.pop_front(), Some(1));
    assert_eq!(deque.pop_front(), Some(2));
    assert_eq!(deque.pop_front(), Some(3));
    assert_eq!(deque.len(), 0);
    assert!(deque.is_empty());
}

#[test]
fn test_as_slices() {
    let mut deque = AltDeque::new();
    deque.push_back(0);
    deque.push_back(1);
    deque.push_back(2);
    assert_eq!(deque.as_slices(), (&[][..], &[0, 1, 2][..]));
    deque.push_front(3);
    deque.push_front(4);
    assert_eq!(deque.as_slices(), (&[4, 3][..], &[0, 1, 2][..]));
}

#[test]
fn test_get() {
    let deque = AltDeque::from(([1, 2], [3, 4]));
    assert_eq!(deque.get(0), Some(&1));
    assert_eq!(deque.get(1), Some(&2));
    assert_eq!(deque.get(2), Some(&3));
    assert_eq!(deque.get(3), Some(&4));
    assert_eq!(deque.get(5), None);
}

#[test]
fn test_reserve_and_exact() {
    let mut deque = AltDeque::from([1, 2, 3, 4]);
    deque.reserve_exact(3);
    assert_eq!(deque.capacity(), 7);
    deque.reserve(4);
    assert_eq!(deque.capacity(), 14);
}

#[test]
fn test_resize() {
    let mut deque = AltDeque::from([1, 2, 3]);
    deque.resize(2, 5);
    assert_eq!(deque, [1, 2]);
    deque.resize(5, 5);
    assert_eq!(deque, [1, 2, 5, 5, 5]);
}

#[test]
fn test_resize_with() {
    let mut deque = AltDeque::from([1, 2, 3]);
    deque.resize_with(2, || unreachable!());
    assert_eq!(deque, [1, 2]);
    deque.resize_with(5, || 2 + 3);
    assert_eq!(deque, [1, 2, 5, 5, 5]);
}

#[test]
fn test_shrink() {
    let mut deque = AltDeque::<i8>::new();
    deque.push_front(-1);
    deque.push_back(1);
    assert_eq!(deque.as_slices(), (&[-1][..], &[1][..]));
    assert_eq!(deque.capacity(), 8);
    deque.shrink_to(0);
    assert_eq!(deque.as_slices(), (&[-1][..], &[1][..]));
    assert_eq!(deque.capacity(), 2);
}

#[test]
fn test_truncate() {
    use std::rc::Rc;

    let el_2 = Rc::new(2);
    let el_4 = Rc::new(4);
    let weak_2 = Rc::downgrade(&el_2);
    let weak_4 = Rc::downgrade(&el_4);
    {
        let mut deque = AltDeque::from(([Rc::new(1), el_2, Rc::new(3)], [el_4]));
        deque.truncate(1);
        assert_eq!(deque.as_slices(), (&[Rc::new(1)][..], &[][..]));
    }
    // check that the truncated elements have been dropped
    assert!(weak_2.upgrade().is_none());
    assert!(weak_4.upgrade().is_none());
}

#[test]
fn test_clear() {
    let mut deque = AltDeque::from([1, 2, 3]);
    deque.clear();
    assert!(deque.is_empty());
}

#[test]
fn test_contains() {
    let deque = AltDeque::from(([1, 2], [3, 4]));
    assert!(deque.contains(&1));
    assert!(deque.contains(&3));
    assert!(!deque.contains(&5));
}

#[test]
fn test_front() {
    let mut deque = AltDeque::new();
    deque.push_back(1);
    deque.push_back(2);
    assert_eq!(deque.front(), Some(&1));
    deque.push_front(3);
    deque.push_front(4);
    assert_eq!(deque.front(), Some(&4));
    deque.clear();
    assert_eq!(deque.front(), None);

    let mut deque = AltDeque::new();
    deque.push_back(1);
    deque.push_back(2);
    assert_eq!(deque.front_mut(), Some(&mut 1));
    deque.push_front(3);
    deque.push_front(4);
    assert_eq!(deque.front_mut(), Some(&mut 4));
    deque.clear();
    assert_eq!(deque.front_mut(), None);
}

#[test]
fn test_back() {
    let mut deque = AltDeque::new();
    deque.push_front(1);
    deque.push_front(2);
    assert_eq!(deque.back(), Some(&1));
    deque.push_back(3);
    deque.push_back(4);
    assert_eq!(deque.back(), Some(&4));
    deque.clear();
    assert_eq!(deque.back(), None);

    let mut deque = AltDeque::new();
    deque.push_front(1);
    deque.push_front(2);
    assert_eq!(deque.back_mut(), Some(&mut 1));
    deque.push_back(3);
    deque.push_back(4);
    assert_eq!(deque.back_mut(), Some(&mut 4));
    deque.clear();
    assert_eq!(deque.back_mut(), None);
}

#[test]
fn test_pop_front() {
    let mut deque = AltDeque::new();
    deque.push_back(1);
    deque.push_back(2);
    deque.push_back(3);
    assert_eq!(deque.pop_front(), Some(1));
    assert_eq!(deque.pop_front(), Some(2));
    assert_eq!(deque.as_slices(), (&[3][..], &[][..]));
}

#[test]
fn test_pop_back() {
    let mut deque = AltDeque::new();
    deque.push_front(1);
    deque.push_front(2);
    deque.push_front(3);
    assert_eq!(deque.pop_back(), Some(1));
    assert_eq!(deque.pop_back(), Some(2));
    assert_eq!(deque.as_slices(), (&[][..], &[3][..]));
}

#[test]
fn test_push_front() {
    let mut deque = AltDeque::new();
    deque.push_front(1);
    deque.push_front(2);
    deque.push_front(3);
    assert_eq!(deque.as_slices(), (&[3, 2, 1][..], &[][..]));
}

#[test]
fn test_push_back() {
    let mut deque = AltDeque::new();
    deque.push_back(1);
    deque.push_back(2);
    deque.push_back(3);
    assert_eq!(deque.as_slices(), (&[][..], &[1, 2, 3][..]));
}

#[test]
fn test_swap() {
    let mut deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    deque.swap(0, 2);
    assert_eq!(deque, [-1, -2, -3, 1, 2, 3]);
    deque.swap(3, 5);
    assert_eq!(deque, [-1, -2, -3, 3, 2, 1]);
    deque.swap(2, 3);
    assert_eq!(deque, [-1, -2, 3, -3, 2, 1]);
    deque.swap(0, 0);
    assert_eq!(deque, [-1, -2, 3, -3, 2, 1]);
}
#[test]
#[should_panic="index out of bounds: the len is 3 but the index is 3"]
fn test_swap_out_of_bounds() {
    let mut deque = AltDeque::from([1, 2, 3]);
    deque.swap(0, deque.len());
}
#[test]
#[should_panic="index out of bounds: the len is 3 but the index is 3"]
fn test_swap_out_of_bounds2() {
    let mut deque = AltDeque::from([1, 2, 3]);
    deque.swap(deque.len(), 0);
}

#[test]
fn test_swap_remove_front() {
    let mut deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    assert_eq!(deque.swap_remove_front(2), Some(-1));
    assert_eq!(deque, [-2, -3, 1, 2, 3]);
    assert_eq!(deque.swap_remove_front(2), Some(1));
    assert_eq!(deque, [-3, -2, 2, 3]);
    assert_eq!(deque.swap_remove_front(0), Some(-3));
    assert_eq!(deque, [-2, 2, 3]);
    assert_eq!(deque.swap_remove_front(2), Some(3));
    assert_eq!(deque, [2, -2]);
    assert_eq!(deque.swap_remove_front(2), None);
}

#[test]
fn test_swap_remove_back() {
    let mut deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    assert_eq!(deque.swap_remove_back(2), Some(-1));
    assert_eq!(deque, [-3, -2, 3, 1, 2]);
    assert_eq!(deque.swap_remove_back(2), Some(3));
    assert_eq!(deque, [-3, -2, 2, 1]);
    assert_eq!(deque.swap_remove_back(0), Some(-3));
    assert_eq!(deque, [1, -2, 2]);
    assert_eq!(deque.swap_remove_back(2), Some(2));
    assert_eq!(deque, [1, -2]);
    assert_eq!(deque.swap_remove_back(2), None);
}

#[test]
fn test_remove() {
    let mut deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    assert_eq!(deque.remove(2), Some(-1));
    assert_eq!(deque, [-3, -2, 1, 2, 3]);
    assert_eq!(deque.remove(2), Some(1));
    assert_eq!(deque, [-3, -2, 2, 3]);
    assert_eq!(deque.remove(0), Some(-3));
    assert_eq!(deque, [-2, 2, 3]);
    assert_eq!(deque.remove(2), Some(3));
    assert_eq!(deque, [-2, 2]);
    assert_eq!(deque.remove(2), None);
}

#[test]
fn test_insert() {
    let mut deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    deque.insert(0, 4);
    assert_eq!(deque, [4, -3, -2, -1, 1, 2, 3]);
    deque.insert(3, 5);
    assert_eq!(deque, [4, -3, -2, 5, -1, 1, 2, 3]);
    deque.insert(5, 6);
    assert_eq!(deque, [4, -3, -2, 5, -1, 6, 1, 2, 3]);
    deque.insert(9, 7);
    assert_eq!(deque, [4, -3, -2, 5, -1, 6, 1, 2, 3, 7]);
}
#[test]
#[should_panic="index out of bounds: the len is 3 but the index is 4"]
fn test_insert_out_of_bounds() {
    let mut deque = AltDeque::from([1, 2, 3]);
    deque.insert(deque.len() + 1, 42);
}

#[test]
fn test_split_off() {
    let mut deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    let other = deque.split_off(6);
    assert_eq!(deque.as_slices(), (&[-3, -2, -1][..], &[1, 2, 3][..]));
    assert_eq!(other.as_slices(), (&[][..], &[][..]));
    let other = deque.split_off(5);
    assert_eq!(deque.as_slices(), (&[-3, -2, -1][..], &[1, 2][..]));
    assert_eq!(other.as_slices(), (&[3][..], &[][..]));
    let other = deque.split_off(2);
    assert_eq!(deque.as_slices(), (&[-3, -2][..], &[][..]));
    assert_eq!(other.as_slices(), (&[-1, 1, 2][..], &[][..]));
    let other = deque.split_off(0);
    assert_eq!(deque.as_slices(), (&[][..], &[][..]));
    assert_eq!(other.as_slices(), (&[-3, -2][..], &[][..]));
}
#[test]
#[should_panic="index out of bounds: the len is 3 but the index is 4"]
fn test_split_off_out_of_bounds() {
    let mut deque = AltDeque::from([1, 2, 3]);
    let _splitter = deque.split_off(4);
}

#[test]
fn test_append() {
    let mut deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    let mut other = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    deque.append(&mut other);
    assert_eq!(deque.as_slices(), (&[-3, -2, -1][..], &[1, 2, 3, -3, -2, -1, 1, 2, 3][..]));
    assert!(other.is_empty());
}
#[test]
#[should_panic="capacity overflow"]
fn test_append_overflow() {
    // using more than isize::MAX here would trigger a compiler bug
    // see https://github.com/rust-lang/rust/issues/34127
    let mut deque = AltDeque::from([(); isize::MAX as usize]);
    deque.append(&mut deque.clone());
    deque.append(&mut deque.clone());
}

#[test]
fn test_retain() {
    let mut deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    deque.retain(|el| el % 2 == 0);
    assert_eq!(deque, [-2, 2]);
}

#[test]
fn test_retain_mut() {
    let mut deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    deque.retain_mut(|el| { *el += 1; *el % 2 == 0 });
    assert_eq!(deque, [-2, 0, 2, 4]);
}

#[test]
fn test_make_contiguous() {
    let mut deque = AltDeque::new();
    deque.push_back(1);
    deque.push_back(2);
    // everything is in the back stack
    assert_eq!(deque.make_contiguous(), &[1, 2][..]);
    // everything is in the front stack
    assert_eq!(deque.make_contiguous(), &[1, 2][..]);

    deque.push_back(3);
    deque.push_back(4);
    deque.reserve(2);
    // there is enough space to copy the back stack
    assert_eq!(deque.make_contiguous(), &[1, 2, 3, 4][..]);

    deque.push_back(5);
    deque.push_back(6);
    deque.push_back(7);
    deque.push_back(8);
    deque.pop_front();
    deque.pop_front();
    // there is enough space to copy the front stack but not to copy the back stack
    assert_eq!(deque.capacity(), 8);
    assert_eq!(deque.make_contiguous(), &[3, 4, 5, 6, 7, 8][..]);

    deque.push_back(9);
    deque.push_back(10);
    // there is neither enough space to copy the front stack nor the back stack
    assert_eq!(deque.capacity(), 8);
    assert_eq!(deque.make_contiguous(), &[3, 4, 5, 6, 7, 8, 9, 10][..]);

    // examples from the comments in the make_contiguous function
    // CDEFGHI..AB
    let mut deque = AltDeque::from(([1, 2], [3, 4, 5, 6, 7, 8, 9, 10, 11]));
    deque.pop_back();
    deque.pop_back();
    assert_eq!(deque.make_contiguous(), &[1, 2, 3, 4, 5, 6, 7, 8, 9][..]);
    // EFGHIJ.ABCD
    let mut deque = AltDeque::from(([1, 2, 3, 4], [5, 6, 7, 8, 9, 10, 11]));
    deque.pop_back();
    assert_eq!(deque.make_contiguous(), &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10][..]);
    // CDEFGH.AB
    let mut deque = AltDeque::from(([1, 2], [3, 4, 5, 6, 7, 8, 9, 10]));
    deque.pop_back();
    assert_eq!(deque.make_contiguous(), &[1, 2, 3, 4, 5, 6, 7, 8, 9][..]);
}

#[test]
fn test_rotate() {
    // just test every possible combination of front len, back len and mid in 0..10
    for left_count in 0..10 {
        for right_count in 0..10 {
            let len = left_count + right_count;
            for mid in 0..len + 1 {
                let mut deque = AltDeque::new();
                let vec_l: Vec<_> = (mid..len).chain(0..mid).collect();
                let vec_r: Vec<_> = (len - mid..len).chain(0..len - mid).collect();
                for i in (0..left_count).rev() {
                    deque.push_front(i);
                }
                for i in left_count..len {
                    deque.push_back(i);
                }
                let mut deque_l = deque.clone();
                let mut deque_r = deque;
                deque_l.rotate_left(mid);
                deque_r.rotate_right(mid);
                assert_eq!(deque_l, vec_l, "left_count: {}, right_count: {}, mid: {}", left_count, right_count, mid);
                assert_eq!(deque_r, vec_r, "left_count: {}, right_count: {}, k: {}", left_count, right_count, mid);
            }
        }
    }
}

#[test]
fn test_binary_search() {
    let deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    assert_eq!(deque.binary_search(&-3), Ok(0));
    assert_eq!(deque.binary_search(&-1), Ok(2));
    assert_eq!(deque.binary_search(&-5), Err(0));
    assert_eq!(deque.binary_search(&0), Err(3));
    assert_eq!(deque.binary_search(&5), Err(6));
}

#[test]
fn test_binary_search_by() {
    let deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    assert_eq!(deque.binary_search_by(|x| x.cmp(&-3)), Ok(0));
    assert_eq!(deque.binary_search_by(|x| x.cmp(&-1)), Ok(2));
    assert_eq!(deque.binary_search_by(|x| x.cmp(&-5)), Err(0));
    assert_eq!(deque.binary_search_by(|x| x.cmp(&0)), Err(3));
    assert_eq!(deque.binary_search_by(|x| x.cmp(&5)), Err(6));
}

#[test]
fn test_binary_search_by_key() {
    let deque = AltDeque::from(([(0, -3), (0, -2), (0, -1)], [(0, 1), (0, 2), (0, 3)]));
    assert_eq!(deque.binary_search_by_key(&-3, |x| x.1), Ok(0));
    assert_eq!(deque.binary_search_by_key(&-1, |x| x.1), Ok(2));
    assert_eq!(deque.binary_search_by_key(&-5, |x| x.1), Err(0));
    assert_eq!(deque.binary_search_by_key(&0, |x| x.1), Err(3));
    assert_eq!(deque.binary_search_by_key(&5, |x| x.1), Err(6));
}

#[test]
fn test_partition_point() {
    let deque = AltDeque::from(([1, 3, 5], [7, 9, 11]));
    assert_eq!(deque.partition_point(|&x| x < 1), 0);
    assert_eq!(deque.partition_point(|&x| x < 5), 2);
    assert_eq!(deque.partition_point(|&x| x < 7), 3);
    assert_eq!(deque.partition_point(|&x| x < 50), 6);
}

#[test]
fn test_iter() {
    let deque = AltDeque::<i32>::from(([-3, -2, -1], [1, 2, 3]));

    assert_eq!(deque.iter().copied().collect::<Vec<_>>(), vec![-3, -2, -1, 1, 2, 3]);
    assert_eq!(deque.iter().skip(4).copied().collect::<Vec<_>>(), vec![2, 3]);

    assert_eq!(deque.iter().size_hint(), (6, Some(6)));
    assert_eq!(deque.iter().skip(4).size_hint(), (2, Some(2)));

    assert_eq!(deque.iter().fold(0, |acc, x| acc + x.abs()), 12);
    assert_eq!(deque.iter().skip(4).fold(0, |acc, x| acc + x.abs()), 5);

    let mut iter = deque.iter();
    assert_eq!((iter.nth(0), iter.nth(0)), (Some(&-3), Some(&-2)));
    assert_eq!((iter.nth(2), iter.nth(2)), (Some(&2), None));

    assert_eq!(deque.iter().last(), Some(&3));
}

#[test]
fn test_iter_mut() {
    let mut deque = AltDeque::<i32>::from(([-3, -2, -1], [1, 2, 3]));
    deque.iter_mut().for_each(|el| *el *= 2);
    assert_eq!(deque, [-6, -4, -2, 2, 4, 6]);
    deque.iter_mut().skip(4).for_each(|el| *el *= 2);
    assert_eq!(deque, [-6, -4, -2, 2, 8, 12]);

    assert_eq!(deque.iter_mut().skip(4).size_hint(), (2, Some(2)));

    assert_eq!(deque.iter_mut().fold(0, |acc, x| acc + x.abs()), 34);
    assert_eq!(deque.iter_mut().skip(4).fold(0, |acc, x| acc + x.abs()), 20);

    let mut iter = deque.iter_mut();
    assert_eq!((iter.nth(0), iter.nth(0)), (Some(&mut -6), Some(&mut -4)));
    assert_eq!((iter.nth(2), iter.nth(2)), (Some(&mut 8), None));

    assert_eq!(deque.iter().last(), Some(&12));
}

#[test]
fn test_range() {
    let deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    assert_eq!(deque.range(..).copied().collect::<Vec<_>>(), [-3, -2, -1, 1, 2, 3]);
    assert_eq!(deque.range(2..4).copied().collect::<Vec<_>>(), [-1, 1]);
    assert_eq!(deque.range(4..).copied().collect::<Vec<_>>(), [2, 3]);
    assert_eq!(deque.range(..2).copied().collect::<Vec<_>>(), [-3, -2]);
}

#[test]
fn test_range_mut() {
    let mut deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    assert_eq!(deque.range_mut(..).map(|el| *el).collect::<Vec<_>>(), [-3, -2, -1, 1, 2, 3]);
    assert_eq!(deque.range_mut(2..4).map(|el| *el).collect::<Vec<_>>(), [-1, 1]);
    assert_eq!(deque.range_mut(4..).map(|el| *el).collect::<Vec<_>>(), [2, 3]);
    assert_eq!(deque.range_mut(..2).map(|el| *el).collect::<Vec<_>>(), [-3, -2]);
}

#[test]
fn test_drain() {
    let mut deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));

    let mut deque2 = deque.clone();
    {
        let drain = deque2.drain(..);
        assert_eq!(drain.collect::<Vec<_>>(), vec![-3, -2, -1, 1, 2, 3]);
    }
    assert_eq!(deque2.into_iter().collect::<Vec<_>>(), vec![]);

    let mut deque2 = deque.clone();
    {
        let drain = deque2.drain(1..5);
        assert_eq!(drain.collect::<Vec<_>>(), vec![-2, -1, 1, 2]);
    }
    assert_eq!(deque2.into_iter().collect::<Vec<_>>(), vec![-3, 3]);

    let mut deque2 = deque.clone();
    {
        let drain = deque2.drain(1..2);
        assert_eq!(drain.collect::<Vec<_>>(), vec![-2]);
    }
    assert_eq!(deque2.into_iter().collect::<Vec<_>>(), vec![-3, -1, 1, 2, 3]);

    let mut deque2 = deque.clone();
    {
        let drain = deque2.drain(4..5);
        assert_eq!(drain.collect::<Vec<_>>(), vec![2]);
    }
    assert_eq!(deque2.into_iter().collect::<Vec<_>>(), vec![-3, -2, -1, 1, 3]);

    let mut drain = deque.drain(2..4);
    assert_eq!(drain.size_hint(), (2, Some(2)));
    assert_eq!(drain.next_back(), Some(1));
    assert_eq!(drain.next_back(), Some(-1));
    assert_eq!(drain.next_back(), None);
}
#[test]
#[should_panic]
fn test_drain_out_of_bounds_start() {
    let deque = AltDeque::from([1, 2, 3]);
    let _range = deque.range(4..5);
}
#[test]
#[should_panic]
fn test_drain_out_of_bounds_end() {
    let deque = AltDeque::from([1, 2, 3]);
    let _range = deque.range(0..4);
}
#[test]
#[should_panic]
fn test_drain_invalid_bounds() {
    let deque = AltDeque::from([1, 2, 3]);
    let _range = deque.range(2..1);
}

#[test]
fn test_trait_clone() {
    let deque = AltDeque::from([1, 2, 3]);
    assert_eq!(deque.clone(), [1, 2, 3]);
}

#[test]
fn test_trait_debug() {
    let deque = AltDeque::from(([1, 2, 3], [4, 5, 6]));
    assert_eq!(format!("{:?}", deque), "[1, 2, 3, 4, 5, 6]");
}

#[test]
fn test_trait_default() {
    assert!(AltDeque::<i32>::default().is_empty());
}

#[test]
fn test_trait_drop() {
    use std::rc::Rc;

    let el = Rc::new(1);
    let weak = Rc::downgrade(&el);
    {
        let mut deque = AltDeque::new();
        deque.push_back(Rc::clone(&el));
        deque.push_front(el);
        assert!(weak.upgrade().is_some());
        assert_eq!(weak.upgrade().as_ref(), deque.get(0));
    }
    assert!(weak.upgrade().is_none());
}

#[test]
fn test_trait_extend() {
    let mut deque = AltDeque::new();
    deque.push_front(1);
    deque.push_back(2);
    deque.extend([3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(deque.as_slices(), (&[1][..], &[2, 3, 4, 5, 6, 7, 8, 9][..]));
}

#[test]
fn test_trait_from() {
    // from array
    let deque = AltDeque::from([1, 2, 3]);
    assert_eq!(deque.capacity(), 3);
    assert_eq!(deque.as_slices(), (&[1, 2, 3][..], &[][..]));

    // from tuple of arrays
    let deque = AltDeque::from(([1, 2, 3], [4, 5, 6]));
    assert_eq!(deque.capacity(), 6);
    assert_eq!(deque, [1, 2, 3, 4, 5, 6]);

    // vec from deque
    let mut vec = Vec::from(deque);
    assert_eq!(vec.capacity(), 6);
    assert_eq!(vec, [1, 2, 3, 4, 5, 6]);
    vec.push(7);

    // from vec
    let deque = AltDeque::from(vec);
    assert!(deque.capacity() >= 6);
    assert_eq!(deque, [1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn test_trait_from_iterator() {
    assert_eq!(AltDeque::from_iter([1, 2, 3].into_iter()), [1, 2, 3]);
}

#[test]
fn test_trait_hash() {
    let mut set = std::collections::HashSet::new();
    set.insert(AltDeque::from([1, 2, 3]));
    set.insert(AltDeque::from(([1, 2], [3])));
    assert_eq!(set.len(), 1);
    set.insert(AltDeque::from(([1, 2], [3, 4])));
    assert_eq!(set.len(), 2);
}

#[test]
fn test_trait_index() {
    assert_eq!(AltDeque::from([1, 2, 3])[1], 2);
}
#[test]
#[should_panic="index out of bounds: the len is 3 but the index is 3"]
fn test_trait_index_out_of_bounds() {
    let _ = AltDeque::from([1, 2, 3])[3];
}

#[test]
fn test_trait_index_mut() {
    let mut deque = AltDeque::from([1, 2, 3]);
    deque[1] += 10;
    assert_eq!(deque, [1, 12, 3]);
}
#[test]
#[should_panic="index out of bounds: the len is 3 but the index is 3"]
fn test_trait_index_mut_out_of_bounds() {
    let mut deque = AltDeque::from([1, 2, 3]);
    deque[3] += 10;
}

#[test]
fn test_trait_into_iter() {
    let deque = AltDeque::from(([-3, -2, -1], [1, 2, 3]));
    let iter = deque.into_iter();
    assert_eq!(iter.size_hint(), (6, Some(6)));
    assert_eq!(iter.collect::<Vec<_>>(), vec![-3, -2, -1, 1, 2, 3]);
}

#[test]
fn test_trait_partial_ord() {
    assert_eq!(AltDeque::from([1]).partial_cmp(&AltDeque::from([1])), Some(Ordering::Equal));
    assert_eq!(AltDeque::from([1]).partial_cmp(&AltDeque::from([2])), Some(Ordering::Less));
    assert_eq!(AltDeque::from([2]).partial_cmp(&AltDeque::from([1])), Some(Ordering::Greater));
}

#[test]
fn test_trait_ord() {
    assert_eq!(AltDeque::from([1]).cmp(&AltDeque::from([1])), Ordering::Equal);
    assert_eq!(AltDeque::from([1]).cmp(&AltDeque::from([2])), Ordering::Less);
    assert_eq!(AltDeque::from([2]).cmp(&AltDeque::from([1])), Ordering::Greater);
}

#[test]
fn test_trait_partial_eq() {
    assert_eq!(AltDeque::from(([], [1, 2, 3])), AltDeque::from([1, 2, 3]));
    assert_ne!(AltDeque::from(([], [1, 2, 3])), AltDeque::from([1, 2, 4]));
    assert_eq!(AltDeque::from(([1, 2, 3], [])), AltDeque::from([1, 2, 3]));
    assert_ne!(AltDeque::from(([1, 2, 3], [])), AltDeque::from([1, 2, 4]));
    assert_eq!(AltDeque::from(([1, 2], [3])), AltDeque::from([1, 2, 3]));
    assert_ne!(AltDeque::from(([1, 2], [3])), AltDeque::from([1, 2, 4]));
}
