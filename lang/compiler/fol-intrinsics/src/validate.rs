use crate::IntrinsicEntry;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RegistryValidationErrorKind {
    /// Two entries share an id. Lowering and the backend dispatch on the id, not
    /// the name, so the second entry would be shadowed rather than rejected.
    DuplicateId,
    DuplicateCanonicalName,
    DuplicateAlias,
    AliasMatchesCanonicalName,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RegistryValidationError {
    pub kind: RegistryValidationErrorKind,
    pub left_name: &'static str,
    pub right_name: &'static str,
    pub offending_alias: &'static str,
}

pub fn validate_intrinsic_registry(
    entries: &[IntrinsicEntry],
) -> Result<(), RegistryValidationError> {
    for (index, entry) in entries.iter().enumerate() {
        for alias in entry.aliases {
            if *alias == entry.name {
                return Err(RegistryValidationError {
                    kind: RegistryValidationErrorKind::AliasMatchesCanonicalName,
                    left_name: entry.name,
                    right_name: entry.name,
                    offending_alias: alias,
                });
            }
        }

        for other in &entries[index + 1..] {
            if entry.id.index() == other.id.index() {
                return Err(RegistryValidationError {
                    kind: RegistryValidationErrorKind::DuplicateId,
                    left_name: entry.name,
                    right_name: other.name,
                    offending_alias: "",
                });
            }

            if entry.name == other.name {
                return Err(RegistryValidationError {
                    kind: RegistryValidationErrorKind::DuplicateCanonicalName,
                    left_name: entry.name,
                    right_name: other.name,
                    offending_alias: "",
                });
            }

            for alias in entry.aliases {
                if other.aliases.contains(alias) {
                    return Err(RegistryValidationError {
                        kind: RegistryValidationErrorKind::DuplicateAlias,
                        left_name: entry.name,
                        right_name: other.name,
                        offending_alias: alias,
                    });
                }
                if other.name == *alias {
                    return Err(RegistryValidationError {
                        kind: RegistryValidationErrorKind::AliasMatchesCanonicalName,
                        left_name: entry.name,
                        right_name: other.name,
                        offending_alias: alias,
                    });
                }
            }

            for alias in other.aliases {
                if entry.name == *alias {
                    return Err(RegistryValidationError {
                        kind: RegistryValidationErrorKind::AliasMatchesCanonicalName,
                        left_name: other.name,
                        right_name: entry.name,
                        offending_alias: alias,
                    });
                }
            }
        }
    }

    Ok(())
}
