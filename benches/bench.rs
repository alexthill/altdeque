#![feature(test)]

extern crate test;

use test::Bencher;
use altdeque::AltDeque;
use std::collections::VecDeque;

#[bench]
fn bench_push_and_pop_altdeque(b: &mut Bencher) {
    b.iter(|| {
        let mut deque = AltDeque::new();
        for i in 0..1001 {
            deque.push_back(i);
        }

        let mut sum = 0;
        while let Some(value) = deque.pop_front() {
            sum += value;
        }
        assert_eq!(sum, 500500);
    });
}

#[bench]
fn bench_push_and_pop_vecdeque(b: &mut Bencher) {
    b.iter(|| {
        let mut deque = VecDeque::new();
        for i in 0..1001 {
            deque.push_back(i);
        }

        let mut sum = 0;
        while let Some(value) = deque.pop_front() {
            sum += value;
        }
        assert_eq!(sum, 500500);
    });
}


#[bench]
fn bench_get_altdeque(b: &mut Bencher) {
    let deque = (0..1001).collect::<AltDeque<_>>();
    b.iter(|| {
        let mut sum = 0;
        for i in 0..1001 {
            if let Some(x) = deque.get(i * 2) {
                sum += x;
            }
        }
        assert_eq!(sum, 250500);
    });
}

#[bench]
fn bench_get_vecdeque(b: &mut Bencher) {
    let deque = (0..1001).collect::<VecDeque<_>>();
    b.iter(|| {
        let mut sum = 0;
        for i in 0..1001 {
            if let Some(x) = deque.get(i * 2) {
                sum += x;
            }
        }
        assert_eq!(sum, 250500);
    });
}

#[bench]
fn bench_get_vec(b: &mut Bencher) {
    let vec = (0..1001).collect::<Vec<_>>();
    b.iter(|| {
        let mut sum = 0;
        for i in 0..1001 {
            if let Some(x) = vec.get(i * 2) {
                sum += x;
            }
        }
        assert_eq!(sum, 250500);
    });
}
