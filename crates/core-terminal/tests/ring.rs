use massorca_core_terminal::ring::{ByteRing, RING_CAPACITY};

#[test]
fn empty_ring_reads_nothing_at_offset_zero() {
    let ring = ByteRing::new(8);
    assert_eq!(ring.start_offset(), 0);
    assert_eq!(ring.end_offset(), 0);
    assert_eq!(ring.read_from(0), Some(Vec::new()));
    assert_eq!(ring.read_from(1), None);
}

#[test]
fn offsets_count_every_byte_while_under_capacity() {
    let mut ring = ByteRing::new(10);
    ring.push(b"abcd");
    ring.push(b"ef");
    assert_eq!(ring.start_offset(), 0);
    assert_eq!(ring.end_offset(), 6);
    assert_eq!(ring.read_from(0), Some(b"abcdef".to_vec()));
    assert_eq!(ring.read_from(2), Some(b"cdef".to_vec()));
    assert_eq!(ring.read_from(6), Some(Vec::new()));
    assert_eq!(ring.read_from(7), None);
}

#[test]
fn reads_before_the_start_fail_once_bytes_are_evicted() {
    let mut ring = ByteRing::new(4);
    ring.push(b"abcdef");
    assert_eq!(ring.start_offset(), 2);
    assert_eq!(ring.end_offset(), 6);
    assert_eq!(ring.read_from(1), None);
    assert_eq!(ring.read_from(2), Some(b"cdef".to_vec()));
    assert_eq!(ring.read_from(4), Some(b"ef".to_vec()));
    assert_eq!(ring.read_from(6), Some(Vec::new()));
}

#[test]
fn many_pushes_keep_only_the_newest_capacity_bytes() {
    let capacity = 16;
    let mut ring = ByteRing::new(capacity);
    let mut stream: Vec<u8> = Vec::new();
    for i in 0..1000u32 {
        let chunk: Vec<u8> = (0..7u8)
            .map(|k| (i as u8).wrapping_mul(7).wrapping_add(k))
            .collect();
        ring.push(&chunk);
        stream.extend_from_slice(&chunk);

        let end = stream.len() as u64;
        let kept = &stream[stream.len().saturating_sub(capacity)..];
        let start = end - kept.len() as u64;
        assert_eq!(ring.end_offset(), end, "end after push {i}");
        assert_eq!(ring.start_offset(), start, "start after push {i}");
        assert_eq!(
            ring.read_from(start),
            Some(kept.to_vec()),
            "read at start after push {i}"
        );
        assert_eq!(
            ring.read_from(start + 3),
            Some(kept[3..].to_vec()),
            "read mid after push {i}"
        );
        assert_eq!(
            ring.read_from(end),
            Some(Vec::new()),
            "read at end after push {i}"
        );
        assert_eq!(
            ring.read_from(end + 1),
            None,
            "read past end after push {i}"
        );
        if start > 0 {
            assert_eq!(
                ring.read_from(start - 1),
                None,
                "read before start after push {i}"
            );
        }
    }
}

#[test]
fn push_larger_than_capacity_keeps_only_its_tail() {
    let mut ring = ByteRing::new(8);
    let big: Vec<u8> = (0..20).collect();
    ring.push(&big);
    assert_eq!(ring.start_offset(), 12);
    assert_eq!(ring.end_offset(), 20);
    assert_eq!(ring.read_from(12), Some(big[12..].to_vec()));

    ring.push(&[100, 101, 102]);
    assert_eq!(ring.start_offset(), 15);
    assert_eq!(ring.end_offset(), 23);
    assert_eq!(
        ring.read_from(15),
        Some(vec![15, 16, 17, 18, 19, 100, 101, 102])
    );
}

#[test]
fn zero_length_push_moves_nothing() {
    let mut ring = ByteRing::new(8);
    ring.push(b"xy");
    ring.push(&[]);
    assert_eq!(ring.start_offset(), 0);
    assert_eq!(ring.end_offset(), 2);
    assert_eq!(ring.read_from(0), Some(b"xy".to_vec()));
    assert_eq!(ring.read_from(2), Some(Vec::new()));
}

#[test]
fn zero_capacity_keeps_no_bytes_but_counts_them() {
    let mut ring = ByteRing::new(0);
    ring.push(b"ab");
    assert_eq!(ring.start_offset(), 2);
    assert_eq!(ring.end_offset(), 2);
    assert_eq!(ring.read_from(2), Some(Vec::new()));
    assert_eq!(ring.read_from(0), None);
}

#[test]
fn default_capacity_is_one_mebibyte() {
    assert_eq!(RING_CAPACITY, 1024 * 1024);
    let mut ring = ByteRing::new(RING_CAPACITY);
    let chunk = vec![7u8; 64 * 1024];
    for _ in 0..40 {
        ring.push(&chunk);
    }
    let end = 40 * 64 * 1024;
    assert_eq!(ring.end_offset(), end);
    assert_eq!(ring.start_offset(), end - RING_CAPACITY as u64);
    let kept = ring.read_from(ring.start_offset());
    assert_eq!(kept.map(|bytes| bytes.len()), Some(RING_CAPACITY));
}
