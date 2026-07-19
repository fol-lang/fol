use crate::TypecheckCapabilityModel;
use fol_intrinsics::{intrinsic_registry, IntrinsicStatus, IntrinsicSurface};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorIntrinsicInfo {
    pub name: &'static str,
    pub surface: IntrinsicSurface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorTypeFamily {
    Scalar,
    Array,
    RecordLike,
    OptionalShell,
    ErrorShell,
    Pointer,
    Channel,
    Eventual,
    String,
    Vector,
    Sequence,
    Set,
    Map,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorModelCapability {
    pub heap: bool,
    pub hosted_runtime: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorStructuredTypeInfo {
    pub name: &'static str,
    pub family: EditorTypeFamily,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorProcessorKeywordContext {
    Plain,
    PipeStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorProcessorKeywordInfo {
    pub name: &'static str,
    pub context: EditorProcessorKeywordContext,
}

pub fn editor_declaration_keywords() -> &'static [&'static str] {
    fol_lexer::token::buildin::DECLARATION_KEYWORDS
}

pub fn editor_builtin_type_names() -> &'static [&'static str] {
    crate::BuiltinType::ALL_NAMES
}

pub fn editor_container_type_names() -> &'static [&'static str] {
    fol_parser::CONTAINER_TYPE_NAMES
}

pub fn editor_shell_type_names() -> &'static [&'static str] {
    fol_parser::SHELL_TYPE_NAMES
}

pub fn editor_structured_type_infos() -> &'static [EditorStructuredTypeInfo] {
    const TYPES: &[EditorStructuredTypeInfo] = &[
        EditorStructuredTypeInfo {
            name: "ptr",
            family: EditorTypeFamily::Pointer,
        },
        EditorStructuredTypeInfo {
            name: "chn",
            family: EditorTypeFamily::Channel,
        },
        EditorStructuredTypeInfo {
            name: "evt",
            family: EditorTypeFamily::Eventual,
        },
    ];
    TYPES
}

pub fn editor_processor_keyword_infos() -> &'static [EditorProcessorKeywordInfo] {
    const KEYWORDS: &[EditorProcessorKeywordInfo] = &[
        EditorProcessorKeywordInfo {
            name: "select",
            context: EditorProcessorKeywordContext::Plain,
        },
        EditorProcessorKeywordInfo {
            name: "async",
            context: EditorProcessorKeywordContext::PipeStage,
        },
        EditorProcessorKeywordInfo {
            name: "await",
            context: EditorProcessorKeywordContext::PipeStage,
        },
    ];
    KEYWORDS
}

pub fn editor_source_kind_names() -> &'static [&'static str] {
    fol_parser::SOURCE_KIND_NAMES
}

pub fn editor_implemented_intrinsics() -> Vec<EditorIntrinsicInfo> {
    intrinsic_registry()
        .iter()
        .filter(|entry| entry.status == IntrinsicStatus::Implemented)
        .map(|entry| EditorIntrinsicInfo {
            name: entry.name,
            surface: entry.surface,
        })
        .collect()
}

pub fn editor_model_capability(model: TypecheckCapabilityModel) -> EditorModelCapability {
    match model {
        TypecheckCapabilityModel::Core => EditorModelCapability {
            heap: false,
            hosted_runtime: false,
        },
        TypecheckCapabilityModel::Memo => EditorModelCapability {
            heap: true,
            hosted_runtime: false,
        },
        TypecheckCapabilityModel::Std => EditorModelCapability {
            heap: true,
            hosted_runtime: true,
        },
    }
}

pub fn editor_type_family_available_in_model(
    model: TypecheckCapabilityModel,
    family: EditorTypeFamily,
) -> bool {
    match family {
        EditorTypeFamily::Scalar
        | EditorTypeFamily::Array
        | EditorTypeFamily::RecordLike
        | EditorTypeFamily::OptionalShell
        | EditorTypeFamily::ErrorShell
        | EditorTypeFamily::Pointer => true,
        EditorTypeFamily::Channel | EditorTypeFamily::Eventual => {
            editor_model_capability(model).hosted_runtime
        }
        EditorTypeFamily::String
        | EditorTypeFamily::Vector
        | EditorTypeFamily::Sequence
        | EditorTypeFamily::Set
        | EditorTypeFamily::Map => editor_model_capability(model).heap,
    }
}

pub fn editor_processor_keyword_available_in_model(
    model: TypecheckCapabilityModel,
    _keyword: EditorProcessorKeywordInfo,
) -> bool {
    editor_model_capability(model).hosted_runtime
}

pub fn editor_intrinsic_available_in_model(
    model: TypecheckCapabilityModel,
    intrinsic: EditorIntrinsicInfo,
) -> bool {
    if intrinsic.name == "echo" {
        return editor_model_capability(model).hosted_runtime;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{
        editor_builtin_type_names, editor_container_type_names, editor_declaration_keywords,
        editor_implemented_intrinsics, editor_intrinsic_available_in_model,
        editor_model_capability, editor_processor_keyword_available_in_model,
        editor_processor_keyword_infos, editor_shell_type_names, editor_source_kind_names,
        editor_structured_type_infos, editor_type_family_available_in_model, EditorIntrinsicInfo,
        EditorProcessorKeywordContext, EditorTypeFamily,
    };
    use crate::TypecheckCapabilityModel;
    use fol_intrinsics::{intrinsic_registry, IntrinsicStatus, IntrinsicSurface};

    #[test]
    fn editor_metadata_api_exposes_nonempty_language_facts() {
        assert!(!editor_declaration_keywords().is_empty());
        assert!(!editor_builtin_type_names().is_empty());
        assert!(!editor_container_type_names().is_empty());
        assert!(!editor_shell_type_names().is_empty());
        assert!(!editor_structured_type_infos().is_empty());
        assert!(!editor_processor_keyword_infos().is_empty());
        assert!(!editor_source_kind_names().is_empty());
        assert!(!editor_implemented_intrinsics().is_empty());
    }

    #[test]
    fn editor_model_capabilities_follow_core_mem_std_shape() {
        assert_eq!(
            editor_model_capability(TypecheckCapabilityModel::Core),
            super::EditorModelCapability {
                heap: false,
                hosted_runtime: false,
            }
        );
        assert_eq!(
            editor_model_capability(TypecheckCapabilityModel::Memo),
            super::EditorModelCapability {
                heap: true,
                hosted_runtime: false,
            }
        );
        assert_eq!(
            editor_model_capability(TypecheckCapabilityModel::Std),
            super::EditorModelCapability {
                heap: true,
                hosted_runtime: true,
            }
        );
        assert!(!editor_type_family_available_in_model(
            TypecheckCapabilityModel::Core,
            EditorTypeFamily::String
        ));
        assert!(editor_type_family_available_in_model(
            TypecheckCapabilityModel::Memo,
            EditorTypeFamily::String
        ));
    }

    #[test]
    fn editor_keyword_and_type_facts_match_compiler_constants_exactly() {
        assert_eq!(
            editor_declaration_keywords(),
            fol_lexer::token::buildin::DECLARATION_KEYWORDS
        );
        assert_eq!(editor_builtin_type_names(), crate::BuiltinType::ALL_NAMES);
        assert_eq!(
            editor_container_type_names(),
            fol_parser::CONTAINER_TYPE_NAMES
        );
        assert_eq!(editor_shell_type_names(), fol_parser::SHELL_TYPE_NAMES);
        assert_eq!(editor_source_kind_names(), fol_parser::SOURCE_KIND_NAMES);
        assert_eq!(
            editor_structured_type_infos()
                .iter()
                .map(|info| info.name)
                .collect::<Vec<_>>(),
            ["ptr", "chn", "evt"]
        );
        assert_eq!(
            editor_processor_keyword_infos()
                .iter()
                .map(|info| (info.name, info.context))
                .collect::<Vec<_>>(),
            [
                ("select", EditorProcessorKeywordContext::Plain),
                ("async", EditorProcessorKeywordContext::PipeStage),
                ("await", EditorProcessorKeywordContext::PipeStage),
            ]
        );
        for info in editor_processor_keyword_infos() {
            assert!(
                fol_lexer::token::buildin::CONTROL_KEYWORDS.contains(&info.name)
                    || fol_lexer::token::buildin::OTHER_KEYWORDS.contains(&info.name),
                "processor completion keyword '{}' must be compiler-lexed",
                info.name
            );
        }
    }

    #[test]
    fn editor_intrinsic_facts_match_implemented_registry_entries_exactly() {
        let mut from_editor: Vec<_> = editor_implemented_intrinsics().into_iter().collect();
        from_editor.sort_by_key(|info| (info.name, format!("{:?}", info.surface)));
        let mut from_registry: Vec<_> = intrinsic_registry()
            .iter()
            .filter(|entry| entry.status == IntrinsicStatus::Implemented)
            .map(|entry| EditorIntrinsicInfo {
                name: entry.name,
                surface: entry.surface,
            })
            .collect();
        from_registry.sort_by_key(|info| (info.name, format!("{:?}", info.surface)));
        assert_eq!(from_editor, from_registry);
    }

    #[test]
    fn editor_intrinsic_model_policy_keeps_echo_std_only() {
        let echo = EditorIntrinsicInfo {
            name: "echo",
            surface: IntrinsicSurface::DotRootCall,
        };
        let len = EditorIntrinsicInfo {
            name: "len",
            surface: IntrinsicSurface::DotRootCall,
        };

        assert!(!editor_intrinsic_available_in_model(
            TypecheckCapabilityModel::Core,
            echo
        ));
        assert!(!editor_intrinsic_available_in_model(
            TypecheckCapabilityModel::Memo,
            echo
        ));
        assert!(editor_intrinsic_available_in_model(
            TypecheckCapabilityModel::Std,
            echo
        ));

        assert!(editor_intrinsic_available_in_model(
            TypecheckCapabilityModel::Core,
            len
        ));
    }

    #[test]
    fn compiler_owned_model_matrix_locks_runtime_capability_contract() {
        let echo = EditorIntrinsicInfo {
            name: "echo",
            surface: IntrinsicSurface::DotRootCall,
        };
        let len = EditorIntrinsicInfo {
            name: "len",
            surface: IntrinsicSurface::DotRootCall,
        };

        let matrix = [
            (
                TypecheckCapabilityModel::Core,
                super::EditorModelCapability {
                    heap: false,
                    hosted_runtime: false,
                },
                [
                    (EditorTypeFamily::Scalar, true),
                    (EditorTypeFamily::Array, true),
                    (EditorTypeFamily::RecordLike, true),
                    (EditorTypeFamily::OptionalShell, true),
                    (EditorTypeFamily::ErrorShell, true),
                    (EditorTypeFamily::Pointer, true),
                    (EditorTypeFamily::Channel, false),
                    (EditorTypeFamily::String, false),
                    (EditorTypeFamily::Vector, false),
                    (EditorTypeFamily::Sequence, false),
                    (EditorTypeFamily::Set, false),
                    (EditorTypeFamily::Map, false),
                ],
                [(echo, false), (len, true)],
            ),
            (
                TypecheckCapabilityModel::Memo,
                super::EditorModelCapability {
                    heap: true,
                    hosted_runtime: false,
                },
                [
                    (EditorTypeFamily::Scalar, true),
                    (EditorTypeFamily::Array, true),
                    (EditorTypeFamily::RecordLike, true),
                    (EditorTypeFamily::OptionalShell, true),
                    (EditorTypeFamily::ErrorShell, true),
                    (EditorTypeFamily::Pointer, true),
                    (EditorTypeFamily::Channel, false),
                    (EditorTypeFamily::String, true),
                    (EditorTypeFamily::Vector, true),
                    (EditorTypeFamily::Sequence, true),
                    (EditorTypeFamily::Set, true),
                    (EditorTypeFamily::Map, true),
                ],
                [(echo, false), (len, true)],
            ),
            (
                TypecheckCapabilityModel::Std,
                super::EditorModelCapability {
                    heap: true,
                    hosted_runtime: true,
                },
                [
                    (EditorTypeFamily::Scalar, true),
                    (EditorTypeFamily::Array, true),
                    (EditorTypeFamily::RecordLike, true),
                    (EditorTypeFamily::OptionalShell, true),
                    (EditorTypeFamily::ErrorShell, true),
                    (EditorTypeFamily::Pointer, true),
                    (EditorTypeFamily::Channel, true),
                    (EditorTypeFamily::String, true),
                    (EditorTypeFamily::Vector, true),
                    (EditorTypeFamily::Sequence, true),
                    (EditorTypeFamily::Set, true),
                    (EditorTypeFamily::Map, true),
                ],
                [(echo, true), (len, true)],
            ),
        ];

        for (model, expected_capability, families, intrinsics) in matrix {
            assert_eq!(editor_model_capability(model), expected_capability);
            for (family, expected) in families {
                assert_eq!(
                    editor_type_family_available_in_model(model, family),
                    expected,
                    "type family availability drifted for model={} family={family:?}",
                    model.as_str()
                );
            }
            for (intrinsic, expected) in intrinsics {
                assert_eq!(
                    editor_intrinsic_available_in_model(model, intrinsic),
                    expected,
                    "intrinsic availability drifted for model={} intrinsic={}",
                    model.as_str(),
                    intrinsic.name
                );
            }
            for keyword in editor_processor_keyword_infos() {
                assert_eq!(
                    editor_processor_keyword_available_in_model(model, *keyword),
                    expected_capability.hosted_runtime,
                    "processor keyword availability drifted for model={} keyword={}",
                    model.as_str(),
                    keyword.name
                );
            }
        }
    }
}
