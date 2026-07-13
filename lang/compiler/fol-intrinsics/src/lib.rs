//! Shared intrinsic registry foundations for the FOL compiler.

mod catalog;
mod diagnostics;
mod families;
mod model;
mod registry;
mod select;
mod validate;

pub const CRATE_NAME: &str = "fol-intrinsics";

pub use catalog::{
    all_intrinsics, backend_role_for_intrinsic, implemented_intrinsics_for_backend_role,
    intrinsic_by_alias, intrinsic_by_canonical_name, intrinsic_by_id, intrinsic_registry,
    intrinsics_for_lowering_mode, intrinsics_for_roadmap, intrinsics_for_surface,
    is_reserved_intrinsic_name_for_surface, lowering_mode_for_intrinsic,
    reserved_intrinsic_for_surface, roadmap_for_intrinsic,
};
pub use diagnostics::{
    unknown_intrinsic_message, unsupported_intrinsic_message, wrong_arity_message,
    wrong_type_family_message, wrong_version_message,
};
pub use families::{
    boolean_operand_contract, comparison_operand_contract, query_operand_contract,
    BooleanOperandContract, ComparisonOperandContract, QueryOperandContract,
};
pub use model::{
    IntrinsicAvailability, IntrinsicBackendRole, IntrinsicCategory, IntrinsicId, IntrinsicRoadmap,
    IntrinsicStatus, IntrinsicSurface,
};
pub use registry::{IntrinsicArity, IntrinsicEntry, IntrinsicLoweringMode};
pub use select::{select_intrinsic, IntrinsicSelectionError, IntrinsicSelectionErrorKind};
pub use validate::{
    validate_intrinsic_registry, RegistryValidationError, RegistryValidationErrorKind,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_expected_foundation_identity() {
        assert_eq!(CRATE_NAME, "fol-intrinsics");
    }

    #[test]
    fn public_model_types_cover_basic_intrinsic_dimensions() {
        assert_eq!(IntrinsicId::new(3).index(), 3);
        assert_eq!(IntrinsicCategory::Comparison.as_str(), "comparison");
        assert_eq!(IntrinsicSurface::DotRootCall.as_str(), "dot-root-call");
        assert_eq!(IntrinsicAvailability::V1.as_str(), "V1");
        assert_eq!(IntrinsicRoadmap::CurrentV1.as_str(), "current-v1");
        assert_eq!(IntrinsicBackendRole::PureOp.as_str(), "pure-op");
        assert_eq!(IntrinsicStatus::Implemented.as_str(), "implemented");
    }

    #[test]
    fn intrinsic_entries_capture_registry_metadata() {
        const ENTRY: IntrinsicEntry = IntrinsicEntry::new(
            IntrinsicId::new(0),
            "eq",
            &["equal"],
            IntrinsicCategory::Comparison,
            IntrinsicSurface::DotRootCall,
            IntrinsicAvailability::V1,
            IntrinsicStatus::Implemented,
            IntrinsicArity::Exactly(2),
            IntrinsicLoweringMode::GeneralIr,
            "compare two values for equality",
        );

        assert_eq!(ENTRY.name, "eq");
        assert_eq!(ENTRY.aliases, &["equal"]);
        assert_eq!(ENTRY.arity, IntrinsicArity::Exactly(2));
        assert_eq!(ENTRY.lowering_mode, IntrinsicLoweringMode::GeneralIr);
    }

    #[test]
    fn canonical_registry_contains_expected_first_batch_and_deferred_entries() {
        let names: Vec<_> = intrinsic_registry()
            .iter()
            .map(|entry| entry.name)
            .collect();

        assert!(names.contains(&"eq"));
        assert!(names.contains(&"not"));
        assert!(names.contains(&"len"));
        assert!(names.contains(&"echo"));
        assert!(names.contains(&"de_alloc"));
        assert!(!names.contains(&"pointer_value"));
        assert!(!names.contains(&"give_back"));
        assert!(!names.contains(&"address_of"));
        assert!(!names.contains(&"borrow_from"));
    }

    #[test]
    fn lookup_apis_find_intrinsics_by_name_alias_and_surface() {
        let eq = intrinsic_by_canonical_name("eq").expect("eq should exist");
        let nq = intrinsic_by_alias("ne").expect("ne alias should exist");
        let dot_calls = intrinsics_for_surface(IntrinsicSurface::DotRootCall);

        assert_eq!(eq.name, "eq");
        assert_eq!(nq.name, "nq");
        assert!(dot_calls.iter().any(|entry| entry.name == "len"));
    }

    #[test]
    fn parser_facing_helpers_identify_reserved_names_by_surface() {
        let len = reserved_intrinsic_for_surface(IntrinsicSurface::DotRootCall, "len")
            .expect("len should be reserved for dot-root intrinsics");

        assert_eq!(len.name, "len");
        assert!(is_reserved_intrinsic_name_for_surface(
            IntrinsicSurface::KeywordCall,
            "panic"
        ));
        assert!(!is_reserved_intrinsic_name_for_surface(
            IntrinsicSurface::DotRootCall,
            "user_helper"
        ));
    }

    #[test]
    fn selection_api_distinguishes_unknown_names_from_surface_mismatches() {
        let eq = select_intrinsic(IntrinsicSurface::DotRootCall, "eq")
            .expect("eq should select on the dot-root surface");
        let wrong_surface = select_intrinsic(IntrinsicSurface::DotRootCall, "panic")
            .expect_err("panic should be keyword-only");
        let unknown = select_intrinsic(IntrinsicSurface::DotRootCall, "user_helper")
            .expect_err("unknown helpers should stay unknown");

        assert_eq!(eq.name, "eq");
        assert_eq!(
            wrong_surface.kind,
            IntrinsicSelectionErrorKind::WrongSurface
        );
        assert_eq!(wrong_surface.name, "panic");
        assert_eq!(unknown.kind, IntrinsicSelectionErrorKind::UnknownName);
    }

    #[test]
    fn lowering_apis_track_modes_by_id_and_family() {
        let eq = intrinsic_by_canonical_name("eq").expect("eq should exist");
        let runtime_hooks = intrinsics_for_lowering_mode(IntrinsicLoweringMode::RuntimeHook);

        assert_eq!(
            lowering_mode_for_intrinsic(eq.id),
            Some(IntrinsicLoweringMode::GeneralIr)
        );
        assert_eq!(intrinsic_by_id(eq.id).map(|entry| entry.name), Some("eq"));
        assert!(runtime_hooks.iter().any(|entry| entry.name == "echo"));
    }

    #[test]
    fn comparison_family_registry_entries_stay_stable() {
        let expected = [
            ("eq", IntrinsicId::new(0)),
            ("nq", IntrinsicId::new(1)),
            ("lt", IntrinsicId::new(2)),
            ("gt", IntrinsicId::new(3)),
            ("ge", IntrinsicId::new(4)),
            ("le", IntrinsicId::new(5)),
        ];

        for (name, id) in expected {
            let entry = intrinsic_by_canonical_name(name)
                .unwrap_or_else(|| panic!("comparison intrinsic '{name}' should exist"));
            assert_eq!(entry.id, id);
            assert_eq!(entry.category, IntrinsicCategory::Comparison);
            assert_eq!(entry.surface, IntrinsicSurface::DotRootCall);
            assert_eq!(entry.availability, IntrinsicAvailability::V1);
            assert_eq!(entry.status, IntrinsicStatus::Implemented);
            assert_eq!(entry.arity, IntrinsicArity::Exactly(2));
            assert_eq!(entry.lowering_mode, IntrinsicLoweringMode::GeneralIr);
        }

        assert_eq!(intrinsic_by_alias("ne").map(|entry| entry.name), Some("nq"));
    }

    #[test]
    fn boolean_family_registry_entries_stay_stable() {
        let not = intrinsic_by_canonical_name("not").expect("not should exist");

        assert_eq!(not.id, IntrinsicId::new(6));
        assert_eq!(not.category, IntrinsicCategory::Boolean);
        assert_eq!(not.surface, IntrinsicSurface::DotRootCall);
        assert_eq!(not.availability, IntrinsicAvailability::V1);
        assert_eq!(not.status, IntrinsicStatus::Implemented);
        assert_eq!(not.arity, IntrinsicArity::Exactly(1));
        assert_eq!(not.lowering_mode, IntrinsicLoweringMode::GeneralIr);
    }

    #[test]
    fn length_family_registry_entries_stay_stable() {
        let len = intrinsic_by_canonical_name("len").expect("len should exist");

        assert_eq!(len.id, IntrinsicId::new(7));
        assert_eq!(len.category, IntrinsicCategory::Query);
        assert_eq!(len.surface, IntrinsicSurface::DotRootCall);
        assert_eq!(len.availability, IntrinsicAvailability::V1);
        assert_eq!(len.status, IntrinsicStatus::Implemented);
        assert_eq!(len.arity, IntrinsicArity::Exactly(1));
        assert_eq!(len.lowering_mode, IntrinsicLoweringMode::DedicatedIr);
    }

    #[test]
    fn diagnostic_family_registry_entries_stay_stable() {
        let echo = intrinsic_by_canonical_name("echo").expect("echo should exist");

        assert_eq!(echo.id, IntrinsicId::new(8));
        assert_eq!(echo.category, IntrinsicCategory::Diagnostic);
        assert_eq!(echo.surface, IntrinsicSurface::DotRootCall);
        assert_eq!(echo.availability, IntrinsicAvailability::V1);
        assert_eq!(echo.status, IntrinsicStatus::Implemented);
        assert_eq!(echo.arity, IntrinsicArity::Exactly(1));
        assert_eq!(echo.lowering_mode, IntrinsicLoweringMode::RuntimeHook);
        assert_eq!(
            intrinsic_by_alias("print").map(|entry| entry.name),
            Some("echo")
        );
        assert_eq!(
            echo.doc_summary,
            "emit a runtime-visible debug value and forward it unchanged"
        );
    }

    #[test]
    fn explicit_deallocation_stays_at_the_v4_ffi_boundary() {
        let entry = intrinsic_by_canonical_name("de_alloc")
            .expect("explicit deallocation boundary should remain registered");
        assert_eq!(entry.id, IntrinsicId::new(16));
        assert_eq!(entry.category, IntrinsicCategory::Memory);
        assert_eq!(entry.surface, IntrinsicSurface::DotRootCall);
        assert_eq!(entry.availability, IntrinsicAvailability::V4);
        assert_eq!(entry.status, IntrinsicStatus::Unsupported);
        assert_eq!(entry.arity, IntrinsicArity::Exactly(1));
        assert_eq!(entry.lowering_mode, IntrinsicLoweringMode::Reject);
    }

    #[test]
    fn deferred_query_and_arithmetic_registry_entries_stay_stable() {
        let expected = [
            (
                "low",
                IntrinsicId::new(21),
                IntrinsicCategory::Query,
                IntrinsicArity::Exactly(1),
            ),
            (
                "high",
                IntrinsicId::new(22),
                IntrinsicCategory::Query,
                IntrinsicArity::Exactly(1),
            ),
            (
                "min",
                IntrinsicId::new(23),
                IntrinsicCategory::Arithmetic,
                IntrinsicArity::Exactly(2),
            ),
            (
                "max",
                IntrinsicId::new(24),
                IntrinsicCategory::Arithmetic,
                IntrinsicArity::Exactly(2),
            ),
            (
                "clamp",
                IntrinsicId::new(25),
                IntrinsicCategory::Arithmetic,
                IntrinsicArity::Exactly(3),
            ),
        ];

        for (name, id, category, arity) in expected {
            let entry = intrinsic_by_canonical_name(name)
                .unwrap_or_else(|| panic!("deferred intrinsic '{name}' should exist"));
            assert_eq!(entry.id, id);
            assert_eq!(entry.category, category);
            assert_eq!(entry.surface, IntrinsicSurface::DotRootCall);
            assert_eq!(entry.availability, IntrinsicAvailability::V1);
            assert_eq!(entry.status, IntrinsicStatus::Unsupported);
            assert_eq!(entry.arity, arity);
            assert_eq!(entry.lowering_mode, IntrinsicLoweringMode::Deferred);
        }
    }

    #[test]
    fn query_and_arithmetic_classification_stays_explicit_for_current_v1_scope() {
        let len = intrinsic_by_canonical_name("len").expect("len should exist");
        let deferred = ["cap", "is_empty", "low", "high", "min", "max", "clamp"];

        assert_eq!(len.category, IntrinsicCategory::Query);
        assert_eq!(len.availability, IntrinsicAvailability::V1);
        assert_eq!(len.status, IntrinsicStatus::Implemented);
        assert_eq!(len.lowering_mode, IntrinsicLoweringMode::DedicatedIr);

        for name in deferred {
            let entry = intrinsic_by_canonical_name(name)
                .unwrap_or_else(|| panic!("deferred intrinsic '{name}' should exist"));
            assert_eq!(entry.availability, IntrinsicAvailability::V1);
            assert_eq!(entry.status, IntrinsicStatus::Unsupported);
            assert_eq!(entry.lowering_mode, IntrinsicLoweringMode::Deferred);
        }
    }

    #[test]
    fn conversion_operator_registry_entries_stay_stable() {
        let expected = [("cast", IntrinsicId::new(9)), ("as", IntrinsicId::new(10))];

        for (name, id) in expected {
            let entry = intrinsic_by_canonical_name(name)
                .unwrap_or_else(|| panic!("conversion intrinsic '{name}' should exist"));
            assert_eq!(entry.id, id);
            assert_eq!(entry.category, IntrinsicCategory::Conversion);
            assert_eq!(entry.surface, IntrinsicSurface::OperatorAlias);
            assert_eq!(entry.availability, IntrinsicAvailability::V1);
            assert_eq!(entry.status, IntrinsicStatus::Unsupported);
            assert_eq!(entry.arity, IntrinsicArity::Exactly(2));
            assert_eq!(entry.lowering_mode, IntrinsicLoweringMode::Reject);
        }
    }

    #[test]
    fn conversion_intrinsics_stay_deferred_for_current_v1_scope() {
        for name in ["cast", "as"] {
            let entry = intrinsic_by_canonical_name(name)
                .unwrap_or_else(|| panic!("conversion intrinsic '{name}' should exist"));
            assert_eq!(entry.availability, IntrinsicAvailability::V1);
            assert_eq!(entry.status, IntrinsicStatus::Unsupported);
            assert_eq!(entry.lowering_mode, IntrinsicLoweringMode::Reject);
        }
    }

    #[test]
    fn keyword_intrinsics_are_selected_from_the_same_registry_model() {
        let panic_entry = select_intrinsic(IntrinsicSurface::KeywordCall, "panic")
            .expect("panic should select on the keyword surface");
        let check_entry = select_intrinsic(IntrinsicSurface::KeywordCall, "check")
            .expect("check should select on the keyword surface");

        assert_eq!(panic_entry.name, "panic");
        assert_eq!(check_entry.name, "check");
        assert_eq!(panic_entry.surface, IntrinsicSurface::KeywordCall);
        assert_eq!(check_entry.surface, IntrinsicSurface::KeywordCall);
        assert!(select_intrinsic(IntrinsicSurface::DotRootCall, "panic").is_err());
        assert!(select_intrinsic(IntrinsicSurface::DotRootCall, "check").is_err());
    }

    #[test]
    fn keyword_intrinsic_registry_entries_stay_stable() {
        let check = intrinsic_by_canonical_name("check").expect("check should exist");
        let panic_entry = intrinsic_by_canonical_name("panic").expect("panic should exist");

        assert_eq!(check.id, IntrinsicId::new(12));
        assert_eq!(check.category, IntrinsicCategory::Recoverable);
        assert_eq!(check.surface, IntrinsicSurface::KeywordCall);
        assert_eq!(check.availability, IntrinsicAvailability::V1);
        assert_eq!(check.status, IntrinsicStatus::Implemented);
        assert_eq!(check.arity, IntrinsicArity::Exactly(1));
        assert_eq!(check.lowering_mode, IntrinsicLoweringMode::DedicatedIr);

        assert_eq!(panic_entry.id, IntrinsicId::new(13));
        assert_eq!(panic_entry.category, IntrinsicCategory::Diagnostic);
        assert_eq!(panic_entry.surface, IntrinsicSurface::KeywordCall);
        assert_eq!(panic_entry.availability, IntrinsicAvailability::V1);
        assert_eq!(panic_entry.status, IntrinsicStatus::Implemented);
        assert_eq!(panic_entry.arity, IntrinsicArity::AtLeast(0));
        assert_eq!(
            panic_entry.lowering_mode,
            IntrinsicLoweringMode::DedicatedIr
        );
    }

    #[test]
    fn arithmetic_bitwise_and_overflow_roadmap_entries_stay_registered() {
        let expected = [
            (
                "add",
                IntrinsicId::new(26),
                IntrinsicCategory::Arithmetic,
                IntrinsicArity::Exactly(2),
            ),
            (
                "abs",
                IntrinsicId::new(30),
                IntrinsicCategory::Arithmetic,
                IntrinsicArity::Exactly(1),
            ),
            (
                "sqrt",
                IntrinsicId::new(36),
                IntrinsicCategory::Arithmetic,
                IntrinsicArity::Exactly(1),
            ),
            (
                "bit_and",
                IntrinsicId::new(37),
                IntrinsicCategory::Bitwise,
                IntrinsicArity::Exactly(2),
            ),
            (
                "pop_count",
                IntrinsicId::new(44),
                IntrinsicCategory::Bitwise,
                IntrinsicArity::Exactly(1),
            ),
            (
                "checked_add",
                IntrinsicId::new(49),
                IntrinsicCategory::Arithmetic,
                IntrinsicArity::Exactly(2),
            ),
            (
                "overflowing_sub",
                IntrinsicId::new(56),
                IntrinsicCategory::Arithmetic,
                IntrinsicArity::Exactly(2),
            ),
        ];

        for (name, id, category, arity) in expected {
            let entry = intrinsic_by_canonical_name(name)
                .unwrap_or_else(|| panic!("roadmap intrinsic '{name}' should exist"));
            assert_eq!(entry.id, id);
            assert_eq!(entry.category, category);
            assert_eq!(entry.surface, IntrinsicSurface::DotRootCall);
            assert_eq!(entry.availability, IntrinsicAvailability::V1);
            assert_eq!(entry.status, IntrinsicStatus::Unsupported);
            assert_eq!(entry.arity, arity);
            assert_eq!(entry.lowering_mode, IntrinsicLoweringMode::Deferred);
        }
    }

    #[test]
    fn roadmap_lookup_keeps_representative_intrinsics_explicit() {
        let len = intrinsic_by_canonical_name("len").expect("len should exist");
        let cast = intrinsic_by_canonical_name("cast").expect("cast should exist");
        let add = intrinsic_by_canonical_name("add").expect("add should exist");
        let bit_and = intrinsic_by_canonical_name("bit_and").expect("bit_and should exist");
        let de_alloc = intrinsic_by_canonical_name("de_alloc").expect("de_alloc should exist");

        assert_eq!(
            roadmap_for_intrinsic(len.id),
            Some(IntrinsicRoadmap::CurrentV1)
        );
        assert_eq!(
            roadmap_for_intrinsic(cast.id),
            Some(IntrinsicRoadmap::LikelyV1x)
        );
        assert_eq!(
            roadmap_for_intrinsic(add.id),
            Some(IntrinsicRoadmap::CoreStdInstead)
        );
        assert_eq!(
            roadmap_for_intrinsic(bit_and.id),
            Some(IntrinsicRoadmap::V2)
        );
        assert_eq!(
            roadmap_for_intrinsic(de_alloc.id),
            Some(IntrinsicRoadmap::V4)
        );
        assert!(intrinsics_for_roadmap(IntrinsicRoadmap::CurrentV1)
            .iter()
            .any(|entry| entry.name == "echo"));
    }

    #[test]
    fn roadmap_buckets_partition_registry_without_duplicate_spellings() {
        use std::collections::BTreeSet;

        let mut canonical_names = BTreeSet::new();
        let mut aliases = BTreeSet::new();

        for roadmap in [
            IntrinsicRoadmap::CurrentV1,
            IntrinsicRoadmap::LikelyV1x,
            IntrinsicRoadmap::V2,
            IntrinsicRoadmap::V4,
            IntrinsicRoadmap::CoreStdInstead,
        ] {
            let entries = intrinsics_for_roadmap(roadmap);
            assert!(
                !entries.is_empty(),
                "roadmap bucket '{}' should expose at least one intrinsic",
                roadmap.as_str()
            );
            for entry in entries {
                assert_eq!(
                    roadmap_for_intrinsic(entry.id),
                    Some(roadmap),
                    "intrinsic '{}' should stay in its selected roadmap bucket",
                    entry.name
                );
                assert!(
                    canonical_names.insert(entry.name),
                    "intrinsic '{}' should appear in only one roadmap bucket",
                    entry.name
                );
                for alias in entry.aliases {
                    assert!(
                        aliases.insert(*alias),
                        "intrinsic alias '{}' should stay unique across roadmap buckets",
                        alias
                    );
                }
            }
        }

        assert_eq!(canonical_names.len(), intrinsic_registry().len());
        assert!(validate_intrinsic_registry(intrinsic_registry()).is_ok());
    }

    #[test]
    fn implemented_intrinsics_expose_backend_roles_explicitly() {
        let eq = intrinsic_by_canonical_name("eq").expect("eq should exist");
        let len = intrinsic_by_canonical_name("len").expect("len should exist");
        let echo = intrinsic_by_canonical_name("echo").expect("echo should exist");
        let panic_entry = intrinsic_by_canonical_name("panic").expect("panic should exist");
        let cast = intrinsic_by_canonical_name("cast").expect("cast should exist");

        assert_eq!(
            backend_role_for_intrinsic(eq.id),
            Some(IntrinsicBackendRole::PureOp)
        );
        assert_eq!(
            backend_role_for_intrinsic(len.id),
            Some(IntrinsicBackendRole::TargetHelper)
        );
        assert_eq!(
            backend_role_for_intrinsic(echo.id),
            Some(IntrinsicBackendRole::RuntimeHook)
        );
        assert_eq!(
            backend_role_for_intrinsic(panic_entry.id),
            Some(IntrinsicBackendRole::ControlEffect)
        );
        assert_eq!(backend_role_for_intrinsic(cast.id), None);
        assert!(
            implemented_intrinsics_for_backend_role(IntrinsicBackendRole::PureOp)
                .iter()
                .any(|entry| entry.name == "eq")
        );
        assert!(
            implemented_intrinsics_for_backend_role(IntrinsicBackendRole::RuntimeHook)
                .iter()
                .any(|entry| entry.name == "echo")
        );
    }

    #[test]
    fn diagnostics_helpers_render_intrinsic_specific_guidance() {
        let eq = intrinsic_by_canonical_name("eq").expect("eq should exist");
        let de_alloc = intrinsic_by_canonical_name("de_alloc").expect("de_alloc should exist");

        assert_eq!(
            unknown_intrinsic_message(IntrinsicSurface::DotRootCall, "mystery"),
            "unknown dot-root intrinsic '.mystery(...)'"
        );
        assert_eq!(
            wrong_arity_message(eq, 1),
            ".eq(...) expects exactly 2 argument(s) but got 1"
        );
        assert_eq!(
            wrong_type_family_message(eq, "two comparable scalar operands", "'str' and 'seq[str]'"),
            ".eq(...) expects two comparable scalar operands but got 'str' and 'seq[str]'"
        );
        assert_eq!(
            wrong_version_message(de_alloc, IntrinsicAvailability::V1),
            ".de_alloc(...) is a V4/FFI boundary and is not part of V3"
        );
        assert_eq!(
            unsupported_intrinsic_message(de_alloc),
            ".de_alloc(...) is not yet supported"
        );
    }

    #[test]
    fn comparison_operand_contracts_stay_stable_for_v1_registry_entries() {
        let eq = intrinsic_by_canonical_name("eq").expect("eq should exist");
        let nq = intrinsic_by_canonical_name("nq").expect("nq should exist");
        let lt = intrinsic_by_canonical_name("lt").expect("lt should exist");
        let gt = intrinsic_by_canonical_name("gt").expect("gt should exist");
        let len = intrinsic_by_canonical_name("len").expect("len should exist");

        assert_eq!(
            comparison_operand_contract(eq),
            Some(ComparisonOperandContract::EqualityScalar)
        );
        assert_eq!(
            comparison_operand_contract(nq),
            Some(ComparisonOperandContract::EqualityScalar)
        );
        assert_eq!(
            comparison_operand_contract(lt),
            Some(ComparisonOperandContract::OrderedScalar)
        );
        assert_eq!(
            comparison_operand_contract(gt),
            Some(ComparisonOperandContract::OrderedScalar)
        );
        assert_eq!(comparison_operand_contract(len), None);
        assert_eq!(
            ComparisonOperandContract::EqualityScalar.expected_operands(),
            "two comparable scalar operands"
        );
        assert_eq!(
            ComparisonOperandContract::OrderedScalar.expected_operands(),
            "two ordered scalar operands"
        );
    }

    #[test]
    fn boolean_operand_contracts_stay_stable_for_v1_registry_entries() {
        let not = intrinsic_by_canonical_name("not").expect("not should exist");
        let len = intrinsic_by_canonical_name("len").expect("len should exist");

        assert_eq!(
            boolean_operand_contract(not),
            Some(BooleanOperandContract::BoolScalar)
        );
        assert_eq!(boolean_operand_contract(len), None);
        assert_eq!(
            BooleanOperandContract::BoolScalar.expected_operands(),
            "one boolean operand"
        );
    }

    #[test]
    fn query_operand_contracts_stay_stable_for_v1_registry_entries() {
        let len = intrinsic_by_canonical_name("len").expect("len should exist");
        let eq = intrinsic_by_canonical_name("eq").expect("eq should exist");

        assert_eq!(
            query_operand_contract(len),
            Some(QueryOperandContract::LengthQueryable)
        );
        assert_eq!(query_operand_contract(eq), None);
        assert_eq!(
            QueryOperandContract::LengthQueryable.expected_operands(),
            "one string, array, vector, sequence, set, or map operand"
        );
    }

    #[test]
    fn registry_validation_accepts_the_canonical_registry() {
        assert!(validate_intrinsic_registry(intrinsic_registry()).is_ok());
    }
}
