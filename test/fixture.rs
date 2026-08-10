//! Scratch directories for the integration suite.
//!
//! The implementation is `fol-testkit`, shared with the member crates' own
//! tests so there is one guard in the tree rather than one per test target.

pub use fol_testkit::TempFixture;
