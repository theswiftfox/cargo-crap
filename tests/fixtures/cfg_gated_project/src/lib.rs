//! Fixture for testing cfg-gated function deduplication.
//!
//! Two variants of `do_thing` guarded by complementary cfg flags.
//! Only the first variant has LCOV data — simulating a build where
//! `feature = "variant_a"` was active.

#[cfg(feature = "variant_a")]
pub fn do_thing(x: i32) -> i32 {
    if x > 0 {
        x + 1
    } else {
        x - 1
    }
}

#[cfg(not(feature = "variant_a"))]
pub fn do_thing(x: i32) -> i32 {
    if x > 0 {
        x * 2
    } else {
        x * -1
    }
}

/// A regular function that is always compiled.
pub fn always_here() -> i32 {
    42
}
