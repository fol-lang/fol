use crate::{mangle_package_module_name, BackendSession};
use fol_resolver::{PackageIdentity, SourceUnitId};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLayoutPlan {
    pub package_identity: PackageIdentity,
    pub module_name: String,
    pub relative_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceLayoutPlan {
    pub package_identity: PackageIdentity,
    pub full_namespace: String,
    pub module_name: String,
    pub relative_file: String,
    pub source_unit_ids: Vec<SourceUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCrateLayoutPlan {
    pub crate_dir_name: String,
    pub cargo_toml_path: String,
    pub src_dir: String,
    pub main_rs_path: String,
    pub package_mod_paths: Vec<String>,
    pub namespace_file_paths: Vec<String>,
}

pub fn plan_package_layouts(session: &BackendSession) -> Vec<PackageLayoutPlan> {
    session
        .package_graph()
        .iter()
        .cloned()
        .map(|package_identity| {
            let module_name = mangle_package_module_name(&package_identity);
            PackageLayoutPlan {
                package_identity,
                relative_dir: format!("src/packages/{module_name}"),
                module_name,
            }
        })
        .collect()
}

pub fn plan_namespace_layouts(session: &BackendSession) -> Vec<NamespaceLayoutPlan> {
    let mut planned = Vec::new();
    for package_identity in session.package_graph() {
        let Some(package) = session.workspace().package(package_identity) else {
            continue;
        };

        let mut by_namespace: BTreeMap<String, Vec<SourceUnitId>> = BTreeMap::new();
        for source_unit in &package.source_units {
            by_namespace
                .entry(source_unit.namespace.clone())
                .or_default()
                .push(source_unit.source_unit_id);
        }

        let namespace_keys = by_namespace.keys().cloned().collect::<Vec<_>>();

        for (namespace, source_unit_ids) in by_namespace {
            let relative_segments = namespace_segments(package_identity, &namespace);
            let module_name = relative_segments
                .last()
                .map(|segment| sanitize_segment(segment))
                .unwrap_or_else(|| "root".to_string());
            let has_child_namespace = namespace_keys.iter().any(|candidate| {
                candidate != &namespace && candidate.starts_with(&format!("{namespace}::"))
            });
            let relative_file = if relative_segments.is_empty() {
                format!("{module_name}.rs")
            } else if has_child_namespace {
                format!("{}/mod.rs", relative_segments.join("/"))
            } else if relative_segments.len() <= 1 {
                format!("{module_name}.rs")
            } else {
                format!(
                    "{}/{}.rs",
                    relative_segments[..relative_segments.len() - 1].join("/"),
                    module_name
                )
            };

            planned.push(NamespaceLayoutPlan {
                package_identity: package_identity.clone(),
                full_namespace: namespace,
                module_name,
                relative_file,
                source_unit_ids,
            });
        }
    }
    planned
}

pub fn plan_generated_crate_layout(session: &BackendSession) -> GeneratedCrateLayoutPlan {
    let package_layouts = plan_package_layouts(session);
    let namespace_layouts = plan_namespace_layouts(session);

    let package_mod_paths = package_layouts
        .iter()
        .map(|plan| format!("{}/mod.rs", plan.relative_dir))
        .collect();
    let namespace_file_paths = namespace_layouts
        .iter()
        .map(|plan| {
            let package_dir = format!(
                "src/packages/{}",
                mangle_package_module_name(&plan.package_identity)
            );
            format!("{package_dir}/{}", plan.relative_file)
        })
        .collect();

    GeneratedCrateLayoutPlan {
        crate_dir_name: session.workspace_identity().crate_dir_name.clone(),
        cargo_toml_path: "Cargo.toml".to_string(),
        src_dir: "src".to_string(),
        main_rs_path: "src/main.rs".to_string(),
        package_mod_paths,
        namespace_file_paths,
    }
}

fn namespace_segments(package_identity: &PackageIdentity, namespace: &str) -> Vec<String> {
    let mut segments = namespace
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if segments
        .first()
        .is_some_and(|segment| segment == &package_identity.display_name)
    {
        segments.remove(0);
    }
    segments
}

fn sanitize_segment(raw: &str) -> String {
    crate::sanitize_backend_ident(raw)
}

#[cfg(test)]
mod tests {
    use super::{plan_generated_crate_layout, plan_namespace_layouts, plan_package_layouts};
    use crate::{
        testing::{distinct_namespaces, sample_lowered_workspace},
        BackendSession,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("fol_backend_layout_{label}_{unique}"))
    }

    #[test]
    fn package_layout_plans_follow_package_graph_order() {
        let session = BackendSession::new(sample_lowered_workspace());

        let plans = plan_package_layouts(&session);

        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].package_identity.display_name, "app");
        assert_eq!(plans[0].module_name, "pkg__entry__app");
        assert_eq!(plans[0].relative_dir, "src/packages/pkg__entry__app");
        assert_eq!(plans[1].package_identity.display_name, "shared");
        assert_eq!(plans[1].module_name, "pkg__local__shared");
        assert_eq!(plans[1].relative_dir, "src/packages/pkg__local__shared");
    }

    #[test]
    fn namespace_layout_plans_group_source_units_by_namespace() {
        let session = BackendSession::new(sample_lowered_workspace());

        let plans = plan_namespace_layouts(&session);

        assert_eq!(plans.len(), 4);
        assert_eq!(plans[0].full_namespace, "app");
        assert_eq!(plans[0].module_name, "root");
        assert_eq!(plans[0].relative_file, "root.rs");
        assert_eq!(plans[1].full_namespace, "app::math");
        assert_eq!(plans[1].module_name, "math");
        assert_eq!(plans[1].relative_file, "math.rs");
        assert_eq!(plans[3].full_namespace, "shared::util");
        assert_eq!(plans[3].module_name, "util");
        assert_eq!(plans[3].relative_file, "util.rs");
    }

    #[test]
    fn generated_crate_layout_plan_anchors_package_and_namespace_paths() {
        let session = BackendSession::new(sample_lowered_workspace());

        let plan = plan_generated_crate_layout(&session);

        assert!(plan.crate_dir_name.starts_with("fol-build-app-"));
        assert_eq!(plan.cargo_toml_path, "Cargo.toml");
        assert_eq!(plan.src_dir, "src");
        assert_eq!(plan.main_rs_path, "src/main.rs");
        assert_eq!(
            plan.package_mod_paths,
            vec![
                "src/packages/pkg__entry__app/mod.rs",
                "src/packages/pkg__local__shared/mod.rs",
            ]
        );
        assert_eq!(
            plan.namespace_file_paths,
            vec![
                "src/packages/pkg__entry__app/root.rs",
                "src/packages/pkg__entry__app/math.rs",
                "src/packages/pkg__local__shared/root.rs",
                "src/packages/pkg__local__shared/util.rs",
            ]
        );
    }

    #[test]
    fn layout_planning_covers_all_known_packages_and_namespaces_without_duplicates() {
        let workspace = sample_lowered_workspace();
        let expected_namespaces = distinct_namespaces(&workspace);
        let session = BackendSession::new(workspace);

        let package_plans = plan_package_layouts(&session);
        let namespace_plans = plan_namespace_layouts(&session);
        let crate_plan = plan_generated_crate_layout(&session);

        let planned_namespaces = namespace_plans
            .iter()
            .map(|plan| plan.full_namespace.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let planned_package_dirs = package_plans
            .iter()
            .map(|plan| plan.relative_dir.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let namespace_files = crate_plan
            .namespace_file_paths
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(package_plans.len(), session.package_graph().len());
        assert_eq!(planned_package_dirs.len(), package_plans.len());
        assert_eq!(planned_namespaces, expected_namespaces);
        assert_eq!(namespace_files.len(), namespace_plans.len());
        assert!(namespace_files.contains("src/packages/pkg__entry__app/root.rs"));
        assert!(namespace_files.contains("src/packages/pkg__entry__app/math.rs"));
        assert!(namespace_files.contains("src/packages/pkg__local__shared/root.rs"));
        assert!(namespace_files.contains("src/packages/pkg__local__shared/util.rs"));
    }

    #[test]
    fn namespace_with_declarations_and_children_uses_mod_rs() {
        let fixture_root = temp_root("nested_namespace_with_parent_items");
        let app_root = fixture_root.join("app");
        fs::create_dir_all(app_root.join("fmt/math")).expect("nested namespace root");
        fs::write(
            app_root.join("main.fol"),
            concat!(
                "fun[] main(): int = {\n",
                "    return fmt::answer() + fmt::math::answer();\n",
                "};\n",
            ),
        )
        .expect("app source");
        fs::write(
            app_root.join("fmt/root.fol"),
            "fun[] answer(): int = {\n    return 7;\n};\n",
        )
        .expect("fmt root");
        fs::write(
            app_root.join("fmt/math/lib.fol"),
            "fun[] answer(): int = {\n    return 9;\n};\n",
        )
        .expect("fmt child");

        let lowered = crate::testing::lowered_workspace_from_entry_path(&app_root);
        let plans = plan_namespace_layouts(&BackendSession::new(lowered));

        assert!(plans
            .iter()
            .any(|plan| plan.full_namespace == "app::fmt" && plan.relative_file == "fmt/mod.rs"));
        assert!(plans
            .iter()
            .any(|plan| plan.full_namespace == "app::fmt::math"
                && plan.relative_file == "fmt/math.rs"));

        let _ = fs::remove_dir_all(&fixture_root);
    }
}
