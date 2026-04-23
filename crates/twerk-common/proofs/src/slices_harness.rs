use twerk_common::slices::{intersect, map_slice};

#[kani::proof]
fn intersect_symmetric() {
    // Use small bounded arrays for symbolic exploration.
    let a0: u8 = kani::any();
    let a1: u8 = kani::any();
    let b0: u8 = kani::any();
    let b1: u8 = kani::any();

    let a = [a0, a1];
    let b = [b0, b1];

    assert_eq!(intersect(&a, &b), intersect(&b, &a));
}

#[kani::proof]
fn intersect_empty_first() {
    let b = [1u8];
    assert!(!intersect::<u8>(&[], &b));
}

#[kani::proof]
fn map_slice_length_preserved() {
    // Small bounded input: 0..=3 elements
    let len: usize = kani::any();
    kani::assume(len <= 3);

    let mut data = [0u8; 3];
    for i in 0..len {
        data[i] = kani::any();
    }

    let input = &data[..len];
    let output: Vec<u8> = map_slice(input, |x: u8| x.wrapping_add(1));
    assert_eq!(output.len(), input.len());
}
