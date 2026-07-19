//! Runtime trait contracts for backend-generated aggregate types.
//!
//! Backend-generated Rust code is expected to:
//!
//! 1. define native `struct`/`enum` shapes for lowered records and entries
//! 2. implement [`FolRecord`] or [`FolEntry`] on those generated types
//! 3. forward display/echo hooks through [`render_record`] or [`render_entry`]
//!
//! Minimal pattern:
//!
//! ```no_run
//! use fol_runtime::memo::{
//!     render_record, FolEchoFormat, FolInt, FolNamedValue, FolRecord,
//! };
//!
//! struct Point {
//!     x: FolInt,
//!     y: FolInt,
//! }
//!
//! impl FolRecord for Point {
//!     fn fol_record_name(&self) -> &'static str {
//!         "Point"
//!     }
//!
//!     fn fol_record_fields(&self) -> Vec<FolNamedValue> {
//!         vec![
//!             FolNamedValue::new("x", self.x.to_string()),
//!             FolNamedValue::new("y", self.y.to_string()),
//!         ]
//!     }
//! }
//!
//! impl FolEchoFormat for Point {
//!     fn fol_echo_format(&self) -> String {
//!         render_record(self)
//!     }
//! }
//!
//! let point = Point { x: 3, y: 7 };
//! assert_eq!(point.fol_echo_format(), "Point { x: 3, y: 7 }");
//! ```

use crate::{
    containers::FolArray,
    memo::{FolMap, FolSeq, FolSet, FolStr, FolVec},
    shell::{FolError, FolOption},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolNamedValue {
    name: &'static str,
    rendered_value: String,
}

impl FolNamedValue {
    pub fn new(name: &'static str, rendered_value: impl Into<String>) -> Self {
        Self {
            name,
            rendered_value: rendered_value.into(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn rendered_value(&self) -> &str {
        &self.rendered_value
    }
}

pub trait FolRecord {
    fn fol_record_name(&self) -> &'static str;

    fn fol_record_fields(&self) -> Vec<FolNamedValue>;
}

pub trait FolEntry {
    fn fol_entry_name(&self) -> &'static str;

    fn fol_entry_variant_name(&self) -> &'static str;

    fn fol_entry_fields(&self) -> Vec<FolNamedValue>;
}

pub trait FolEchoFormat {
    fn fol_echo_format(&self) -> String;
}

pub fn render_echo<T: FolEchoFormat + ?Sized>(value: &T) -> String {
    value.fol_echo_format()
}

fn join_named_values(values: &[FolNamedValue]) -> String {
    values
        .iter()
        .map(|value| format!("{}: {}", value.name(), value.rendered_value()))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn render_record<T: FolRecord>(value: &T) -> String {
    let fields = value.fol_record_fields();
    format!(
        "{} {{ {} }}",
        value.fol_record_name(),
        join_named_values(&fields)
    )
}

pub fn render_record_debug<T: FolRecord>(value: &T) -> String {
    render_record(value)
}

pub fn render_entry<T: FolEntry>(value: &T) -> String {
    let fields = value.fol_entry_fields();
    if fields.is_empty() {
        format!(
            "{}.{}",
            value.fol_entry_name(),
            value.fol_entry_variant_name()
        )
    } else {
        format!(
            "{}.{} {{ {} }}",
            value.fol_entry_name(),
            value.fol_entry_variant_name(),
            join_named_values(&fields)
        )
    }
}

pub fn render_entry_debug<T: FolEntry>(value: &T) -> String {
    render_entry(value)
}

fn join_echo<I>(items: I) -> String
where
    I: IntoIterator<Item = String>,
{
    items.into_iter().collect::<Vec<_>>().join(", ")
}

impl FolEchoFormat for i64 {
    fn fol_echo_format(&self) -> String {
        self.to_string()
    }
}

impl FolEchoFormat for f64 {
    fn fol_echo_format(&self) -> String {
        self.to_string()
    }
}

impl FolEchoFormat for bool {
    fn fol_echo_format(&self) -> String {
        self.to_string()
    }
}

impl FolEchoFormat for char {
    fn fol_echo_format(&self) -> String {
        self.to_string()
    }
}

impl FolEchoFormat for FolStr {
    fn fol_echo_format(&self) -> String {
        self.to_string()
    }
}

impl<T: FolEchoFormat + ?Sized> FolEchoFormat for Box<T> {
    fn fol_echo_format(&self) -> String {
        render_echo(self.as_ref())
    }
}

impl<T: FolEchoFormat + ?Sized> FolEchoFormat for std::rc::Rc<T> {
    fn fol_echo_format(&self) -> String {
        render_echo(self.as_ref())
    }
}

// A weak handle is rendered as an opaque placeholder rather than upgraded and
// formatted: weak edges exist precisely to break shared cycles, so following one
// during echo formatting would recurse forever on those cycles.
impl<T: ?Sized> FolEchoFormat for std::rc::Weak<T> {
    fn fol_echo_format(&self) -> String {
        "weak".to_string()
    }
}

impl<T: ?Sized> FolEchoFormat for std::sync::Weak<T> {
    fn fol_echo_format(&self) -> String {
        "weak".to_string()
    }
}

impl<T: FolEchoFormat, const N: usize> FolEchoFormat for FolArray<T, N> {
    fn fol_echo_format(&self) -> String {
        format!("arr[{}]", join_echo(self.iter().map(render_echo)))
    }
}

impl<T: FolEchoFormat> FolEchoFormat for FolVec<T> {
    fn fol_echo_format(&self) -> String {
        format!(
            "vec[{}]",
            join_echo(self.as_slice().iter().map(render_echo))
        )
    }
}

impl<T: FolEchoFormat> FolEchoFormat for FolSeq<T> {
    fn fol_echo_format(&self) -> String {
        format!(
            "seq[{}]",
            join_echo(self.as_slice().iter().map(render_echo))
        )
    }
}

impl<T: FolEchoFormat + Ord> FolEchoFormat for FolSet<T> {
    fn fol_echo_format(&self) -> String {
        format!(
            "set{{{}}}",
            join_echo(self.as_set().iter().map(render_echo))
        )
    }
}

impl<K: FolEchoFormat + Ord, V: FolEchoFormat> FolEchoFormat for FolMap<K, V> {
    fn fol_echo_format(&self) -> String {
        format!(
            "map{{{}}}",
            join_echo(self.as_map().iter().map(|(key, value)| format!(
                "{}: {}",
                render_echo(key),
                render_echo(value)
            )))
        )
    }
}

impl<T: FolEchoFormat> FolEchoFormat for FolOption<T> {
    fn fol_echo_format(&self) -> String {
        match self {
            FolOption::Some(value) => format!("some({})", render_echo(value)),
            FolOption::Nil => "nil".to_string(),
        }
    }
}

impl<T: FolEchoFormat> FolEchoFormat for FolError<T> {
    fn fol_echo_format(&self) -> String {
        if self.is_nil() {
            "nil".to_string()
        } else {
            format!("err({})", render_echo(self.as_ref()))
        }
    }
}

pub fn module_name() -> &'static str {
    "aggregate"
}

#[cfg(test)]
mod tests {
    use super::{
        render_echo, render_entry, render_entry_debug, render_record, render_record_debug,
        FolEchoFormat, FolEntry, FolNamedValue, FolRecord,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct DemoPoint {
        x: i64,
        y: i64,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum DemoStatus {
        Ok { count: i64 },
        Err { label: &'static str },
        Empty,
    }

    impl FolRecord for DemoPoint {
        fn fol_record_name(&self) -> &'static str {
            "Point"
        }

        fn fol_record_fields(&self) -> Vec<FolNamedValue> {
            vec![
                FolNamedValue::new("x", self.x.to_string()),
                FolNamedValue::new("y", self.y.to_string()),
            ]
        }
    }

    impl FolEchoFormat for DemoPoint {
        fn fol_echo_format(&self) -> String {
            render_record(self)
        }
    }

    #[test]
    fn record_trait_contract_preserves_backend_authored_field_order() {
        let point = DemoPoint { x: 3, y: 7 };
        let fields = point.fol_record_fields();

        assert_eq!(point.fol_record_name(), "Point");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name(), "x");
        assert_eq!(fields[0].rendered_value(), "3");
        assert_eq!(fields[1].name(), "y");
        assert_eq!(fields[1].rendered_value(), "7");
    }

    impl FolEntry for DemoStatus {
        fn fol_entry_name(&self) -> &'static str {
            "Status"
        }

        fn fol_entry_variant_name(&self) -> &'static str {
            match self {
                Self::Ok { .. } => "Ok",
                Self::Err { .. } => "Err",
                Self::Empty => "Empty",
            }
        }

        fn fol_entry_fields(&self) -> Vec<FolNamedValue> {
            match self {
                Self::Ok { count } => vec![FolNamedValue::new("count", count.to_string())],
                Self::Err { label } => vec![FolNamedValue::new("label", label.to_string())],
                Self::Empty => Vec::new(),
            }
        }
    }

    impl FolEchoFormat for DemoStatus {
        fn fol_echo_format(&self) -> String {
            render_entry(self)
        }
    }

    #[test]
    fn entry_trait_contract_preserves_variant_name_and_payload_shape() {
        let ok = DemoStatus::Ok { count: 7 };
        let err = DemoStatus::Err { label: "bad-input" };

        let ok_fields = ok.fol_entry_fields();
        let err_fields = err.fol_entry_fields();

        assert_eq!(ok.fol_entry_name(), "Status");
        assert_eq!(ok.fol_entry_variant_name(), "Ok");
        assert_eq!(ok_fields, vec![FolNamedValue::new("count", "7")]);

        assert_eq!(err.fol_entry_name(), "Status");
        assert_eq!(err.fol_entry_variant_name(), "Err");
        assert_eq!(err_fields, vec![FolNamedValue::new("label", "bad-input")]);
    }

    #[test]
    fn aggregate_render_helpers_freeze_record_and_entry_shapes() {
        let point = DemoPoint { x: 3, y: 7 };
        let ok = DemoStatus::Ok { count: 7 };
        let empty = DemoStatus::Empty;

        assert_eq!(render_record(&point), "Point { x: 3, y: 7 }");
        assert_eq!(render_record_debug(&point), "Point { x: 3, y: 7 }");
        assert_eq!(render_entry(&ok), "Status.Ok { count: 7 }");
        assert_eq!(render_entry_debug(&ok), "Status.Ok { count: 7 }");
        assert_eq!(render_entry(&empty), "Status.Empty");
    }

    #[test]
    fn aggregate_traits_integrate_with_echo_formatting() {
        let point = DemoPoint { x: 1, y: 2 };
        let ok = DemoStatus::Ok { count: 9 };

        assert_eq!(render_echo(&point), "Point { x: 1, y: 2 }");
        assert_eq!(render_echo(&ok), "Status.Ok { count: 9 }");
    }

    #[test]
    fn model_module_example_shapes_show_backend_authorship_pattern() {
        use crate::memo::{
            render_entry, render_record, FolEchoFormat, FolEntry, FolInt, FolNamedValue, FolRecord,
        };

        struct ExamplePoint {
            x: FolInt,
            y: FolInt,
        }

        impl FolRecord for ExamplePoint {
            fn fol_record_name(&self) -> &'static str {
                "Point"
            }

            fn fol_record_fields(&self) -> Vec<FolNamedValue> {
                vec![
                    FolNamedValue::new("x", self.x.to_string()),
                    FolNamedValue::new("y", self.y.to_string()),
                ]
            }
        }

        impl FolEchoFormat for ExamplePoint {
            fn fol_echo_format(&self) -> String {
                render_record(self)
            }
        }

        enum ExampleStatus {
            Ok { count: FolInt },
        }

        impl FolEntry for ExampleStatus {
            fn fol_entry_name(&self) -> &'static str {
                "Status"
            }

            fn fol_entry_variant_name(&self) -> &'static str {
                "Ok"
            }

            fn fol_entry_fields(&self) -> Vec<FolNamedValue> {
                match self {
                    Self::Ok { count } => vec![FolNamedValue::new("count", count.to_string())],
                }
            }
        }

        impl FolEchoFormat for ExampleStatus {
            fn fol_echo_format(&self) -> String {
                render_entry(self)
            }
        }

        let point = ExamplePoint { x: 4, y: 9 };
        let status = ExampleStatus::Ok { count: 2 };

        assert_eq!(point.fol_echo_format(), "Point { x: 4, y: 9 }");
        assert_eq!(status.fol_echo_format(), "Status.Ok { count: 2 }");
    }
}
