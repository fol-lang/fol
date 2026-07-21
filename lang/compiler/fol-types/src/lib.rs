// FOL Types - Shared types, traits, and core abstractions

#[macro_use]
pub mod r#mod;
pub mod target;

// Basic types
pub use r#mod::*;
pub use target::{ResolveTargetError, ResolvedTarget};

pub fn canonical_identifier_key(name: &str) -> String {
    name.chars()
        .filter(|ch| *ch != '_')
        .map(|ch| {
            if ch.is_ascii() {
                ch.to_ascii_lowercase()
            } else {
                ch
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::canonical_identifier_key;

    #[test]
    fn canonical_identifier_key_normalizes_ascii_case_and_underscores() {
        assert_eq!(canonical_identifier_key("Foo_Bar"), "foobar");
        assert_eq!(canonical_identifier_key("foo__bar"), "foobar");
        assert_eq!(canonical_identifier_key("MIXED_Case_Name"), "mixedcasename");
    }

    #[test]
    fn canonical_identifier_key_preserves_non_ascii_while_normalizing_ascii() {
        assert_eq!(canonical_identifier_key("Straße_Name"), "straßename");
        assert_eq!(canonical_identifier_key("Δelta_Name"), "Δeltaname");
    }
}
