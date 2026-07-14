use crate::{IntrinsicArity, IntrinsicAvailability, IntrinsicEntry, IntrinsicSurface};

fn render_intrinsic_surface(surface: IntrinsicSurface) -> &'static str {
    match surface {
        IntrinsicSurface::DotRootCall => "dot-root intrinsic",
        IntrinsicSurface::KeywordCall => "keyword intrinsic",
        IntrinsicSurface::Postfix => "postfix intrinsic",
        IntrinsicSurface::OperatorAlias => "operator-alias intrinsic",
    }
}

fn render_intrinsic_spelling(entry: &IntrinsicEntry) -> String {
    match entry.surface {
        IntrinsicSurface::DotRootCall => format!(".{}(...)", entry.name),
        IntrinsicSurface::KeywordCall => format!("{}(...)", entry.name),
        IntrinsicSurface::Postfix => format!("...{} ", entry.name),
        IntrinsicSurface::OperatorAlias => format!("operator '{}'", entry.name),
    }
}

pub fn unknown_intrinsic_message(surface: IntrinsicSurface, name: &str) -> String {
    match surface {
        IntrinsicSurface::DotRootCall => {
            format!("unknown dot-root intrinsic '.{}(...)'", name)
        }
        IntrinsicSurface::KeywordCall => {
            format!("unknown keyword intrinsic '{}(...)'", name)
        }
        _ => format!("unknown {} '{}'", render_intrinsic_surface(surface), name),
    }
}

pub fn unsupported_intrinsic_message(entry: &IntrinsicEntry) -> String {
    format!("{} is not yet supported", render_intrinsic_spelling(entry))
}

pub fn wrong_arity_message(entry: &IntrinsicEntry, actual: usize) -> String {
    let expected = match entry.arity {
        IntrinsicArity::Exactly(count) => format!("exactly {}", count),
        IntrinsicArity::AtLeast(count) => format!("at least {}", count),
        IntrinsicArity::Between { min, max } => format!("between {} and {}", min, max),
    };

    format!(
        "{} expects {} argument(s) but got {}",
        render_intrinsic_spelling(entry),
        expected,
        actual
    )
}

pub fn wrong_type_family_message(entry: &IntrinsicEntry, expected: &str, actual: &str) -> String {
    format!(
        "{} expects {} but got {}",
        render_intrinsic_spelling(entry),
        expected,
        actual
    )
}

pub fn wrong_version_message(entry: &IntrinsicEntry, _current: IntrinsicAvailability) -> String {
    match entry.availability {
        IntrinsicAvailability::V4 => format!(
            "{} is a V4/FFI boundary and is not part of V3",
            render_intrinsic_spelling(entry)
        ),
        _ => format!(
            "{} is planned for a future release",
            render_intrinsic_spelling(entry)
        ),
    }
}
