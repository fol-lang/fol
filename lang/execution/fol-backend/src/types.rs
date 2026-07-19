use crate::{
    mangle_package_module_name, mangle_type_name, sanitize_backend_ident, BackendError,
    BackendErrorKind, BackendResult,
};
use fol_lower::{
    LoweredBuiltinType, LoweredType, LoweredTypeDecl, LoweredTypeDeclKind, LoweredTypeId,
    LoweredTypeTable, LoweredVariantLayout, LoweredWorkspace,
};
use fol_resolver::PackageIdentity;

pub fn render_rust_type(
    type_table: &LoweredTypeTable,
    type_id: LoweredTypeId,
) -> BackendResult<String> {
    render_rust_type_in_workspace(None, type_table, type_id)
}

pub fn render_rust_type_in_workspace(
    workspace: Option<&LoweredWorkspace>,
    type_table: &LoweredTypeTable,
    type_id: LoweredTypeId,
) -> BackendResult<String> {
    let Some(ty) = type_table.get(type_id) else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("lowered type {:?} is missing from the type table", type_id),
        ));
    };

    match ty {
        LoweredType::Builtin(LoweredBuiltinType::Str) => Ok("rt_model::FolStr".to_string()),
        LoweredType::Builtin(builtin) => Ok(render_builtin_type(*builtin)?.to_string()),
        LoweredType::GenericParameter { name } => Ok(sanitize_backend_ident(name)),
        LoweredType::Named {
            package, symbol, ..
        } => {
            let workspace = workspace.ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "named lowered types require workspace context",
                )
            })?;
            let declaration = workspace
                .package(package)
                .and_then(|package| package.type_decls.get(symbol))
                .ok_or_else(|| {
                    BackendError::new(
                        BackendErrorKind::InvalidInput,
                        "named lowered type lost its declaration",
                    )
                })?;
            Ok(format!(
                "{}::{}",
                render_namespace_module_path(workspace, package, declaration.source_unit_id)?,
                mangle_type_name(package, declaration.runtime_type, &declaration.name)
            ))
        }
        LoweredType::Owned { inner } => Ok(format!(
            "Box<{}>",
            render_rust_type_in_workspace(workspace, type_table, *inner)?
        )),
        LoweredType::Borrowed { inner, mutable } => Ok(format!(
            "&{}{}",
            if *mutable { "mut " } else { "" },
            render_rust_type_in_workspace(workspace, type_table, *inner)?
        )),
        LoweredType::Pointer {
            target,
            shared,
            weak,
            sync,
        } => Ok(format!(
            "{}<{}>",
            if *weak && *sync {
                "std::sync::Weak"
            } else if *weak {
                "std::rc::Weak"
            } else if *shared && *sync {
                "std::sync::Arc"
            } else if *shared {
                "std::rc::Rc"
            } else {
                "Box"
            },
            render_rust_type_in_workspace(workspace, type_table, *target)?
        )),
        LoweredType::Array {
            element_type,
            size: Some(size),
        } => Ok(format!(
            "rt::FolArray<{}, {size}>",
            render_rust_type_in_workspace(workspace, type_table, *element_type)?
        )),
        LoweredType::Array { size: None, .. } => Err(BackendError::new(
            BackendErrorKind::Unsupported,
            "unsized arrays are not supported; use vec[] for dynamic collections",
        )),
        LoweredType::Vector { element_type } => Ok(format!(
            "rt_model::FolVec<{}>",
            render_rust_type_in_workspace(workspace, type_table, *element_type)?
        )),
        LoweredType::Sequence { element_type } => Ok(format!(
            "rt_model::FolSeq<{}>",
            render_rust_type_in_workspace(workspace, type_table, *element_type)?
        )),
        LoweredType::Channel { element_type } => Ok(format!(
            "rt::FolChannel<{}>",
            render_rust_type_in_workspace(workspace, type_table, *element_type)?
        )),
        LoweredType::ChannelSender { element_type } => Ok(format!(
            "rt::FolSender<{}>",
            render_rust_type_in_workspace(workspace, type_table, *element_type)?
        )),
        LoweredType::ChannelReceiver { element_type } => Ok(format!(
            "rt::FolReceiver<{}>",
            render_rust_type_in_workspace(workspace, type_table, *element_type)?
        )),
        LoweredType::Eventual {
            value_type,
            error_type,
        } => {
            let value_type = render_rust_type_in_workspace(workspace, type_table, *value_type)?;
            let payload = match error_type {
                Some(error_type) => format!(
                    "rt::FolRecover<{value_type}, {}>",
                    render_rust_type_in_workspace(workspace, type_table, *error_type)?
                ),
                None => value_type,
            };
            Ok(format!("rt::FolEventual<{payload}>"))
        }
        LoweredType::Set { member_types } => match member_types.as_slice() {
            [member_type] => Ok(format!(
                "rt_model::FolSet<{}>",
                render_rust_type_in_workspace(workspace, type_table, *member_type)?
            )),
            _ => Err(BackendError::new(
                BackendErrorKind::Unsupported,
                "heterogeneous sets are not supported; sets must have a single member type",
            )),
        },
        LoweredType::Map {
            key_type,
            value_type,
        } => Ok(format!(
            "rt_model::FolMap<{}, {}>",
            render_rust_type_in_workspace(workspace, type_table, *key_type)?,
            render_rust_type_in_workspace(workspace, type_table, *value_type)?
        )),
        LoweredType::Optional { inner } => Ok(format!(
            "rt::FolOption<{}>",
            render_rust_type_in_workspace(workspace, type_table, *inner)?
        )),
        LoweredType::Error { inner } => Ok(match inner {
            Some(inner) => format!(
                "rt::FolError<{}>",
                render_rust_type_in_workspace(workspace, type_table, *inner)?
            ),
            None => "rt::FolError<()>".to_string(),
        }),
        LoweredType::Record { .. } | LoweredType::Entry { .. } => {
            render_named_runtime_type(workspace, type_id)
        }
        LoweredType::Routine(routine_type) => {
            let rendered_params = routine_type
                .params
                .iter()
                .map(|param| render_rust_type_in_workspace(workspace, type_table, *param))
                .collect::<BackendResult<Vec<_>>>()?
                .join(", ");
            let return_section = match (routine_type.return_type, routine_type.error_type) {
                (Some(ret), Some(err)) => format!(
                    " -> rt::FolRecover<{}, {}>",
                    render_rust_type_in_workspace(workspace, type_table, ret)?,
                    render_rust_type_in_workspace(workspace, type_table, err)?
                ),
                (Some(ret), None) => format!(
                    " -> {}",
                    render_rust_type_in_workspace(workspace, type_table, ret)?
                ),
                (None, _) => String::new(),
            };
            Ok(format!("fn({rendered_params}){return_section}"))
        }
    }
}

/// Whether a value of `type_id` transitively matches `matches`, resolving
/// nominal `Named` fields through the workspace's type declarations (with cycle
/// detection). Unlike `type_contains`, this crosses `Named` boundaries: a derive
/// bound on an aggregate propagates through shared/box pointers and named
/// fields to the concrete types they reference, so equality/`Eq`-blocking
/// members (weak pointers, floats, routines) must be seen transitively.
fn type_transitively_contains(
    workspace: &LoweredWorkspace,
    type_table: &LoweredTypeTable,
    type_id: LoweredTypeId,
    matches: &dyn Fn(&LoweredType) -> bool,
    visited: &mut std::collections::BTreeSet<fol_resolver::SymbolId>,
) -> bool {
    let Some(ty) = type_table.get(type_id) else {
        return false;
    };
    if matches(ty) {
        return true;
    }
    match ty {
        LoweredType::Pointer { target: inner, .. }
        | LoweredType::Owned { inner }
        | LoweredType::Borrowed { inner, .. }
        | LoweredType::Optional { inner }
        | LoweredType::Array {
            element_type: inner,
            ..
        }
        | LoweredType::Vector {
            element_type: inner,
        }
        | LoweredType::Sequence {
            element_type: inner,
        }
        | LoweredType::Channel {
            element_type: inner,
        }
        | LoweredType::ChannelSender {
            element_type: inner,
        }
        | LoweredType::ChannelReceiver {
            element_type: inner,
        } => type_transitively_contains(workspace, type_table, *inner, matches, visited),
        LoweredType::Set { member_types } => member_types.iter().any(|member| {
            type_transitively_contains(workspace, type_table, *member, matches, visited)
        }),
        LoweredType::Map {
            key_type,
            value_type,
        } => {
            type_transitively_contains(workspace, type_table, *key_type, matches, visited)
                || type_transitively_contains(workspace, type_table, *value_type, matches, visited)
        }
        LoweredType::Error { inner } => inner.is_some_and(|inner| {
            type_transitively_contains(workspace, type_table, inner, matches, visited)
        }),
        LoweredType::Eventual {
            value_type,
            error_type,
        } => {
            type_transitively_contains(workspace, type_table, *value_type, matches, visited)
                || error_type.is_some_and(|error_type| {
                    type_transitively_contains(workspace, type_table, error_type, matches, visited)
                })
        }
        LoweredType::Record { fields, .. } => fields.values().any(|field| {
            type_transitively_contains(workspace, type_table, *field, matches, visited)
        }),
        LoweredType::Entry { variants } => variants.values().flatten().any(|payload| {
            type_transitively_contains(workspace, type_table, *payload, matches, visited)
        }),
        LoweredType::Named {
            package, symbol, ..
        } => {
            // Break cycles: a nominal type reached twice adds nothing new.
            if !visited.insert(*symbol) {
                return false;
            }
            let Some(type_decl) = workspace
                .package(package)
                .and_then(|pkg| pkg.type_decls.get(symbol))
            else {
                return false;
            };
            match &type_decl.kind {
                LoweredTypeDeclKind::Alias { target_type } => type_transitively_contains(
                    workspace,
                    type_table,
                    *target_type,
                    matches,
                    visited,
                ),
                LoweredTypeDeclKind::Record { fields } => fields.iter().any(|field| {
                    type_transitively_contains(
                        workspace,
                        type_table,
                        field.type_id,
                        matches,
                        visited,
                    )
                }),
                LoweredTypeDeclKind::Entry { variants } => variants
                    .iter()
                    .filter_map(|variant| variant.payload_type)
                    .any(|payload| {
                        type_transitively_contains(workspace, type_table, payload, matches, visited)
                    }),
            }
        }
        LoweredType::Builtin(_)
        | LoweredType::GenericParameter { .. }
        | LoweredType::Routine(_) => false,
    }
}

fn aggregate_derives(
    workspace: &LoweredWorkspace,
    type_table: &LoweredTypeTable,
    field_types: impl Iterator<Item = LoweredTypeId>,
    include_default: bool,
) -> String {
    let mut has_float = false;
    let mut has_routine = false;
    let mut has_weak = false;
    // Derive blockers propagate transitively through named/pointer fields (a
    // Rust derive bound on the aggregate reaches every referenced type), so each
    // check crosses `Named` boundaries via the workspace declarations.
    for type_id in field_types {
        if type_transitively_contains(
            workspace,
            type_table,
            type_id,
            &|ty| matches!(ty, LoweredType::Builtin(LoweredBuiltinType::Float)),
            &mut std::collections::BTreeSet::new(),
        ) {
            has_float = true;
        }
        if type_transitively_contains(
            workspace,
            type_table,
            type_id,
            &|ty| matches!(ty, LoweredType::Routine(_)),
            &mut std::collections::BTreeSet::new(),
        ) {
            has_routine = true;
        }
        if type_transitively_contains(
            workspace,
            type_table,
            type_id,
            &|ty| matches!(ty, LoweredType::Pointer { weak: true, .. }),
            &mut std::collections::BTreeSet::new(),
        ) {
            has_weak = true;
        }
    }
    let mut derives = vec!["Debug", "Clone"];
    // A `Weak<T>` field (`std::rc::Weak`/`std::sync::Weak`) implements neither
    // `PartialEq` nor `Eq`, so an aggregate holding one cannot derive equality.
    if !has_weak {
        derives.push("PartialEq");
        if !has_float && !has_routine {
            derives.push("Eq");
        }
    }
    if include_default && !has_routine {
        derives.push("Default");
    }
    format!("#[derive({})]", derives.join(", "))
}

pub fn render_record_definition(
    workspace: &LoweredWorkspace,
    package_identity: &PackageIdentity,
    type_decl: &LoweredTypeDecl,
    type_table: &LoweredTypeTable,
) -> BackendResult<String> {
    let LoweredTypeDeclKind::Record { fields } = &type_decl.kind else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("type declaration '{}' is not a record", type_decl.name),
        ));
    };

    let rendered_fields = fields
        .iter()
        .map(|field| {
            let rendered_type =
                render_rust_type_in_workspace(Some(workspace), type_table, field.type_id)?;
            Ok(format!(
                "    pub {}: {},",
                crate::escape_rust_field_ident(&field.name),
                rendered_type
            ))
        })
        .collect::<BackendResult<Vec<_>>>()?
        .join("\n");

    let derives = aggregate_derives(
        workspace,
        type_table,
        fields.iter().map(|field| field.type_id),
        true,
    );
    Ok(format!(
        "{}\npub struct {} {{\n{}\n}}\n",
        derives,
        mangle_type_name(package_identity, type_decl.runtime_type, &type_decl.name),
        rendered_fields
    ))
}

pub fn render_record_trait_impl(
    package_identity: &PackageIdentity,
    type_decl: &LoweredTypeDecl,
) -> BackendResult<String> {
    let LoweredTypeDeclKind::Record { fields } = &type_decl.kind else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("type declaration '{}' is not a record", type_decl.name),
        ));
    };

    let type_name = mangle_type_name(package_identity, type_decl.runtime_type, &type_decl.name);
    let rendered_fields = fields
        .iter()
        .map(|field| {
            // Render through FolEchoFormat: containers (seq/vec/set/map) have
            // no Display impl, but every runtime value type formats for echo.
            format!(
                "            rt::FolNamedValue::new(\"{}\", rt::FolEchoFormat::fol_echo_format(&self.{})),",
                field.name,
                crate::escape_rust_field_ident(&field.name)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        "impl rt::FolRecord for {type_name} {{\n    fn fol_record_name(&self) -> &'static str {{\n        \"{}\"\n    }}\n\n    fn fol_record_fields(&self) -> Vec<rt::FolNamedValue> {{\n        vec![\n{}\n        ]\n    }}\n}}\n\nimpl rt::FolEchoFormat for {type_name} {{\n    fn fol_echo_format(&self) -> String {{\n        rt::render_record(self)\n    }}\n}}\n\nimpl std::fmt::Display for {type_name} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        write!(f, \"{{}}\", rt::render_record(self))\n    }}\n}}\n",
        type_decl.name,
        rendered_fields
    ))
}

pub fn render_entry_definition(
    workspace: &LoweredWorkspace,
    package_identity: &PackageIdentity,
    type_decl: &LoweredTypeDecl,
    type_table: &LoweredTypeTable,
) -> BackendResult<String> {
    let LoweredTypeDeclKind::Entry { variants } = &type_decl.kind else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("type declaration '{}' is not an entry", type_decl.name),
        ));
    };

    let rendered_variants = variants
        .iter()
        .map(|variant| render_entry_variant(workspace, variant, type_table))
        .collect::<BackendResult<Vec<_>>>()?
        .join("\n");
    let type_name = mangle_type_name(package_identity, type_decl.runtime_type, &type_decl.name);
    let default_variant = render_entry_default_variant(workspace, variants, type_table)?;
    // Entries hand-write their Default impl, so only Eq is conditional.
    let derives = aggregate_derives(
        workspace,
        type_table,
        variants.iter().filter_map(|variant| variant.payload_type),
        false,
    );

    Ok(format!(
        "{derives}\npub enum {type_name} {{\n{rendered_variants}\n}}\n\nimpl Default for {type_name} {{\n    fn default() -> Self {{\n        {default_variant}\n    }}\n}}\n",
    ))
}

pub fn render_entry_trait_impl(
    package_identity: &PackageIdentity,
    type_decl: &LoweredTypeDecl,
) -> BackendResult<String> {
    let LoweredTypeDeclKind::Entry { variants } = &type_decl.kind else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("type declaration '{}' is not an entry", type_decl.name),
        ));
    };

    let type_name = mangle_type_name(package_identity, type_decl.runtime_type, &type_decl.name);
    let match_arms = variants
        .iter()
        .map(render_entry_trait_match_arm)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        "impl rt::FolEntry for {type_name} {{\n    fn fol_entry_name(&self) -> &'static str {{\n        \"{}\"\n    }}\n\n    fn fol_entry_variant_name(&self) -> &'static str {{\n        match self {{\n{}\n        }}\n    }}\n\n    fn fol_entry_fields(&self) -> Vec<rt::FolNamedValue> {{\n        match self {{\n{}\n        }}\n    }}\n}}\n\nimpl rt::FolEchoFormat for {type_name} {{\n    fn fol_echo_format(&self) -> String {{\n        rt::render_entry(self)\n    }}\n}}\n\nimpl std::fmt::Display for {type_name} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        write!(f, \"{{}}\", rt::render_entry(self))\n    }}\n}}\n",
        type_decl.name,
        match_arms,
        variants
            .iter()
            .map(render_entry_field_match_arm)
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

fn render_entry_variant(
    workspace: &LoweredWorkspace,
    variant: &LoweredVariantLayout,
    type_table: &LoweredTypeTable,
) -> BackendResult<String> {
    Ok(match variant.payload_type {
        Some(payload_type) => format!(
            "    {}({}),",
            crate::escape_rust_field_ident(&variant.name),
            render_rust_type_in_workspace(Some(workspace), type_table, payload_type)?
        ),
        None => format!("    {},", crate::escape_rust_field_ident(&variant.name)),
    })
}

fn render_entry_default_variant(
    _workspace: &LoweredWorkspace,
    variants: &[LoweredVariantLayout],
    _type_table: &LoweredTypeTable,
) -> BackendResult<String> {
    let default_variant = variants.first().ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::InvalidInput,
            "entry definitions must retain at least one variant for Rust emission",
        )
    })?;
    Ok(match default_variant.payload_type {
        Some(_payload_type) => format!(
            "Self::{}(Default::default())",
            crate::escape_rust_field_ident(&default_variant.name)
        ),
        None => format!(
            "Self::{}",
            crate::escape_rust_field_ident(&default_variant.name)
        ),
    })
}

fn render_named_runtime_type(
    workspace: Option<&LoweredWorkspace>,
    type_id: LoweredTypeId,
) -> BackendResult<String> {
    let Some(workspace) = workspace else {
        return Err(BackendError::new(
            BackendErrorKind::Unsupported,
            format!(
                "workspace-aware Rust type rendering is required for named runtime type {:?}",
                type_id
            ),
        ));
    };

    for package in workspace.packages() {
        for type_decl in package.type_decls.values() {
            if type_decl.runtime_type == type_id {
                return Ok(format!(
                    "{}::{}",
                    render_namespace_module_path(
                        workspace,
                        &package.identity,
                        type_decl.source_unit_id
                    )?,
                    mangle_type_name(&package.identity, type_decl.runtime_type, &type_decl.name)
                ));
            }
        }
    }

    Err(BackendError::new(
        BackendErrorKind::InvalidInput,
        format!(
            "named lowered runtime type {:?} does not map to any lowered type declaration",
            type_id
        ),
    ))
}

fn render_namespace_module_path(
    workspace: &LoweredWorkspace,
    package_identity: &PackageIdentity,
    source_unit_id: fol_resolver::SourceUnitId,
) -> BackendResult<String> {
    let package = workspace.package(package_identity).ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::InvalidInput,
            format!(
                "package '{}' is missing from workspace",
                package_identity.display_name
            ),
        )
    })?;
    let source_unit = package
        .source_units
        .iter()
        .find(|source_unit| source_unit.source_unit_id == source_unit_id)
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "source unit {:?} is missing from package '{}'",
                    source_unit_id, package_identity.display_name
                ),
            )
        })?;
    let mut segments = source_unit
        .namespace
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(sanitize_backend_ident)
        .collect::<Vec<_>>();
    if segments
        .first()
        .is_some_and(|segment| segment == &sanitize_backend_ident(&package_identity.display_name))
    {
        segments.remove(0);
    }
    let namespace_segment = match segments.as_slice() {
        [] => "root".to_string(),
        parts => parts.join("::"),
    };
    Ok(format!(
        "crate::packages::{}::{}",
        mangle_package_module_name(package_identity),
        namespace_segment
    ))
}

fn render_entry_trait_match_arm(variant: &LoweredVariantLayout) -> String {
    let ident = crate::escape_rust_field_ident(&variant.name);
    match variant.payload_type {
        Some(_) => format!("            Self::{}(..) => \"{}\",", ident, variant.name),
        None => format!("            Self::{} => \"{}\",", ident, variant.name),
    }
}

fn render_entry_field_match_arm(variant: &LoweredVariantLayout) -> String {
    let ident = crate::escape_rust_field_ident(&variant.name);
    match variant.payload_type {
        Some(_) => format!(
            "            Self::{}(payload) => vec![rt::FolNamedValue::new(\"payload\", rt::FolEchoFormat::fol_echo_format(payload))],",
            ident
        ),
        None => format!("            Self::{} => Vec::new(),", ident),
    }
}

fn render_builtin_type(builtin: LoweredBuiltinType) -> BackendResult<&'static str> {
    match builtin {
        LoweredBuiltinType::Int => Ok("rt::FolInt"),
        LoweredBuiltinType::Float => Ok("rt::FolFloat"),
        LoweredBuiltinType::Bool => Ok("rt::FolBool"),
        LoweredBuiltinType::Char => Ok("rt::FolChar"),
        LoweredBuiltinType::Never => Ok("rt::FolNever"),
        LoweredBuiltinType::Str => Err(BackendError::new(
            BackendErrorKind::Internal,
            "string builtin type reached render_builtin_type; should have been handled before dispatch",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        render_entry_definition, render_entry_trait_impl, render_record_definition,
        render_record_trait_impl, render_rust_type, render_rust_type_in_workspace,
    };
    use crate::testing::{package_identity, sample_lowered_workspace};
    use fol_lower::{
        LoweredBuiltinType, LoweredFieldLayout, LoweredRoutineType, LoweredType, LoweredTypeDecl,
        LoweredTypeDeclKind, LoweredTypeTable, LoweredVariantLayout,
    };
    use fol_resolver::{PackageSourceKind, SourceUnitId, SymbolId};

    #[test]
    fn builtin_scalar_type_rendering_uses_backend_owned_runtime_aliases() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let float_id = table.intern_builtin(LoweredBuiltinType::Float);
        let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
        let char_id = table.intern_builtin(LoweredBuiltinType::Char);
        let never_id = table.intern_builtin(LoweredBuiltinType::Never);

        assert_eq!(
            render_rust_type(&table, int_id),
            Ok("rt::FolInt".to_string())
        );
        assert_eq!(
            render_rust_type(&table, float_id),
            Ok("rt::FolFloat".to_string())
        );
        assert_eq!(
            render_rust_type(&table, bool_id),
            Ok("rt::FolBool".to_string())
        );
        assert_eq!(
            render_rust_type(&table, char_id),
            Ok("rt::FolChar".to_string())
        );
        assert_eq!(
            render_rust_type(&table, never_id),
            Ok("rt::FolNever".to_string())
        );
    }

    #[test]
    fn runtime_backed_type_rendering_covers_current_v1_families() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let array_id = table.intern(LoweredType::Array {
            element_type: int_id,
            size: Some(3),
        });
        let vec_id = table.intern(LoweredType::Vector {
            element_type: int_id,
        });
        let seq_id = table.intern(LoweredType::Sequence {
            element_type: str_id,
        });
        let set_id = table.intern(LoweredType::Set {
            member_types: vec![int_id],
        });
        let map_id = table.intern(LoweredType::Map {
            key_type: str_id,
            value_type: int_id,
        });
        let option_id = table.intern(LoweredType::Optional { inner: str_id });
        let error_id = table.intern(LoweredType::Error {
            inner: Some(str_id),
        });

        assert_eq!(
            render_rust_type(&table, str_id),
            Ok("rt_model::FolStr".to_string())
        );
        assert_eq!(
            render_rust_type(&table, array_id),
            Ok("rt::FolArray<rt::FolInt, 3>".to_string())
        );
        assert_eq!(
            render_rust_type(&table, vec_id),
            Ok("rt_model::FolVec<rt::FolInt>".to_string())
        );
        assert_eq!(
            render_rust_type(&table, seq_id),
            Ok("rt_model::FolSeq<rt_model::FolStr>".to_string())
        );
        assert_eq!(
            render_rust_type(&table, set_id),
            Ok("rt_model::FolSet<rt::FolInt>".to_string())
        );
        assert_eq!(
            render_rust_type(&table, map_id),
            Ok("rt_model::FolMap<rt_model::FolStr, rt::FolInt>".to_string())
        );
        assert_eq!(
            render_rust_type(&table, option_id),
            Ok("rt::FolOption<rt_model::FolStr>".to_string())
        );
        assert_eq!(
            render_rust_type(&table, error_id),
            Ok("rt::FolError<rt_model::FolStr>".to_string())
        );
    }

    #[test]
    fn record_definition_rendering_emits_backend_authored_struct_shapes() {
        let mut table = LoweredTypeTable::new();
        let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let record_id = table.intern(LoweredType::Record {
            fields: std::collections::BTreeMap::from([
                ("active".to_string(), bool_id),
                ("name".to_string(), str_id),
            ]),
            finalized: false,
        });
        let decl = LoweredTypeDecl {
            symbol_id: SymbolId(10),
            source_unit_id: SourceUnitId(0),
            name: "User".to_string(),
            runtime_type: record_id,
            kind: LoweredTypeDeclKind::Record {
                fields: vec![
                    LoweredFieldLayout {
                        name: "name".to_string(),
                        type_id: str_id,
                    },
                    LoweredFieldLayout {
                        name: "active".to_string(),
                        type_id: bool_id,
                    },
                ],
            },
        };
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let workspace = sample_lowered_workspace();

        let rendered = render_record_definition(&workspace, &package_identity, &decl, &table)
            .expect("record definition should render");

        assert!(rendered.contains("#[derive(Debug, Clone, PartialEq, Eq, Default)]"));
        assert!(rendered.contains("pub struct ty__pkg__entry__app__t"));
        assert!(rendered.contains("pub name: rt_model::FolStr,"));
        assert!(rendered.contains("pub active: rt::FolBool,"));
    }

    #[test]
    fn record_trait_impl_rendering_emits_runtime_fol_record_contract() {
        let mut table = LoweredTypeTable::new();
        let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let record_id = table.intern(LoweredType::Record {
            fields: std::collections::BTreeMap::from([
                ("active".to_string(), bool_id),
                ("name".to_string(), str_id),
            ]),
            finalized: false,
        });
        let decl = LoweredTypeDecl {
            symbol_id: SymbolId(10),
            source_unit_id: SourceUnitId(0),
            name: "User".to_string(),
            runtime_type: record_id,
            kind: LoweredTypeDeclKind::Record {
                fields: vec![
                    LoweredFieldLayout {
                        name: "name".to_string(),
                        type_id: str_id,
                    },
                    LoweredFieldLayout {
                        name: "active".to_string(),
                        type_id: bool_id,
                    },
                ],
            },
        };
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");

        let rendered = render_record_trait_impl(&package_identity, &decl)
            .expect("record trait impl should render");

        assert!(rendered.contains("impl rt::FolRecord for ty__pkg__entry__app__t"));
        assert!(rendered.contains("fn fol_record_name(&self) -> &'static str"));
        assert!(rendered.contains("\"User\""));
        assert!(rendered.contains(
            "rt::FolNamedValue::new(\"name\", rt::FolEchoFormat::fol_echo_format(&self.name))"
        ));
        assert!(rendered.contains("impl rt::FolEchoFormat for ty__pkg__entry__app__t"));
        assert!(rendered.contains("rt::render_record(self)"));
    }

    #[test]
    fn entry_definition_rendering_emits_backend_authored_enum_shapes() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let entry_id = table.intern(LoweredType::Entry {
            variants: std::collections::BTreeMap::from([
                ("Empty".to_string(), None),
                ("Ok".to_string(), Some(int_id)),
                ("Err".to_string(), Some(str_id)),
            ]),
        });
        let decl = LoweredTypeDecl {
            symbol_id: SymbolId(11),
            source_unit_id: SourceUnitId(0),
            name: "Status".to_string(),
            runtime_type: entry_id,
            kind: LoweredTypeDeclKind::Entry {
                variants: vec![
                    LoweredVariantLayout {
                        name: "Ok".to_string(),
                        payload_type: Some(int_id),
                    },
                    LoweredVariantLayout {
                        name: "Err".to_string(),
                        payload_type: Some(str_id),
                    },
                    LoweredVariantLayout {
                        name: "Empty".to_string(),
                        payload_type: None,
                    },
                ],
            },
        };
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let workspace = sample_lowered_workspace();

        let rendered = render_entry_definition(&workspace, &package_identity, &decl, &table)
            .expect("entry definition should render");

        assert!(rendered.contains("#[derive(Debug, Clone, PartialEq, Eq)]"));
        assert!(rendered.contains("pub enum ty__pkg__entry__app__t"));
        assert!(rendered.contains("Ok(rt::FolInt),"));
        assert!(rendered.contains("Err(rt_model::FolStr),"));
        assert!(rendered.contains("Empty,"));
    }

    #[test]
    fn entry_trait_impl_rendering_emits_runtime_fol_entry_contract() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let entry_id = table.intern(LoweredType::Entry {
            variants: std::collections::BTreeMap::from([
                ("Empty".to_string(), None),
                ("Ok".to_string(), Some(int_id)),
                ("Err".to_string(), Some(str_id)),
            ]),
        });
        let decl = LoweredTypeDecl {
            symbol_id: SymbolId(11),
            source_unit_id: SourceUnitId(0),
            name: "Status".to_string(),
            runtime_type: entry_id,
            kind: LoweredTypeDeclKind::Entry {
                variants: vec![
                    LoweredVariantLayout {
                        name: "Ok".to_string(),
                        payload_type: Some(int_id),
                    },
                    LoweredVariantLayout {
                        name: "Err".to_string(),
                        payload_type: Some(str_id),
                    },
                    LoweredVariantLayout {
                        name: "Empty".to_string(),
                        payload_type: None,
                    },
                ],
            },
        };
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");

        let rendered = render_entry_trait_impl(&package_identity, &decl)
            .expect("entry trait impl should render");

        assert!(rendered.contains("impl rt::FolEntry for ty__pkg__entry__app__t"));
        assert!(rendered.contains("fn fol_entry_name(&self) -> &'static str"));
        assert!(rendered.contains("\"Status\""));
        assert!(rendered.contains("Self::Ok(..) => \"Ok\""));
        assert!(rendered.contains(
            "Self::Err(payload) => vec![rt::FolNamedValue::new(\"payload\", rt::FolEchoFormat::fol_echo_format(payload))]"
        ));
        assert!(rendered.contains("Self::Empty => Vec::new()"));
        assert!(rendered.contains("impl rt::FolEchoFormat for ty__pkg__entry__app__t"));
        assert!(rendered.contains("rt::render_entry(self)"));
    }

    #[test]
    fn combined_type_emission_snapshot_stays_stable_for_current_v1_shapes() {
        let mut table = LoweredTypeTable::new();
        let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let record_id = table.intern(LoweredType::Record {
            fields: std::collections::BTreeMap::from([
                ("active".to_string(), bool_id),
                ("name".to_string(), str_id),
            ]),
            finalized: false,
        });
        let entry_id = table.intern(LoweredType::Entry {
            variants: std::collections::BTreeMap::from([
                ("Empty".to_string(), None),
                ("Ok".to_string(), Some(int_id)),
                ("Err".to_string(), Some(str_id)),
            ]),
        });
        let record_decl = LoweredTypeDecl {
            symbol_id: SymbolId(10),
            source_unit_id: SourceUnitId(0),
            name: "User".to_string(),
            runtime_type: record_id,
            kind: LoweredTypeDeclKind::Record {
                fields: vec![
                    LoweredFieldLayout {
                        name: "name".to_string(),
                        type_id: str_id,
                    },
                    LoweredFieldLayout {
                        name: "active".to_string(),
                        type_id: bool_id,
                    },
                ],
            },
        };
        let entry_decl = LoweredTypeDecl {
            symbol_id: SymbolId(11),
            source_unit_id: SourceUnitId(0),
            name: "Status".to_string(),
            runtime_type: entry_id,
            kind: LoweredTypeDeclKind::Entry {
                variants: vec![
                    LoweredVariantLayout {
                        name: "Ok".to_string(),
                        payload_type: Some(int_id),
                    },
                    LoweredVariantLayout {
                        name: "Err".to_string(),
                        payload_type: Some(str_id),
                    },
                    LoweredVariantLayout {
                        name: "Empty".to_string(),
                        payload_type: None,
                    },
                ],
            },
        };
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let workspace = sample_lowered_workspace();

        let snapshot = [
            render_record_definition(&workspace, &package_identity, &record_decl, &table)
                .expect("record definition should render"),
            render_record_trait_impl(&package_identity, &record_decl)
                .expect("record trait impl should render"),
            render_entry_definition(&workspace, &package_identity, &entry_decl, &table)
                .expect("entry definition should render"),
            render_entry_trait_impl(&package_identity, &entry_decl)
                .expect("entry trait impl should render"),
        ]
        .join("\n");

        assert!(snapshot.contains("pub struct ty__pkg__entry__app__t"));
        assert!(snapshot.contains("pub enum ty__pkg__entry__app__t"));
        assert!(snapshot.contains("pub name: rt_model::FolStr,"));
        assert!(snapshot.contains("Ok(rt::FolInt),"));
        assert!(snapshot.contains("impl rt::FolRecord"));
        assert!(snapshot.contains("impl rt::FolEntry"));
        assert!(snapshot.contains("rt::render_record(self)"));
        assert!(snapshot.contains("rt::render_entry(self)"));
    }

    #[test]
    fn named_runtime_types_render_through_workspace_paths() {
        let workspace = sample_lowered_workspace();
        let entry_package = workspace.entry_package();
        let user_decl = entry_package
            .type_decls
            .values()
            .find(|decl| decl.name == "User")
            .expect("sample workspace should include User record");

        let rendered = render_rust_type_in_workspace(
            Some(&workspace),
            workspace.type_table(),
            user_decl.runtime_type,
        )
        .expect("named runtime type should render through its module path");

        assert_eq!(
            rendered,
            "crate::packages::pkg__entry__app::root::ty__pkg__entry__app__t3__user"
        );
    }

    #[test]
    fn v1_boundary_types_produce_explicit_rejection_messages() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);

        let unsized_array_id = table.intern(LoweredType::Array {
            element_type: int_id,
            size: None,
        });
        let heterogeneous_set_id = table.intern(LoweredType::Set {
            member_types: vec![int_id, int_id],
        });
        let unsized_err = render_rust_type(&table, unsized_array_id)
            .expect_err("unsized arrays should be rejected");
        assert!(
            unsized_err.message().contains("unsized arrays"),
            "unsized array error should mention unsized arrays"
        );

        let het_set_err = render_rust_type(&table, heterogeneous_set_id)
            .expect_err("heterogeneous sets should be rejected");
        assert!(
            het_set_err.message().contains("heterogeneous sets"),
            "heterogeneous set error should mention heterogeneous sets"
        );
    }

    #[test]
    fn routine_type_rendering_emits_function_pointer_syntax() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);

        let plain_fn_id = table.intern(LoweredType::Routine(LoweredRoutineType {
            params: vec![int_id, str_id],
            return_type: Some(bool_id),
            error_type: None,
        }));
        let void_fn_id = table.intern(LoweredType::Routine(LoweredRoutineType {
            params: vec![int_id],
            return_type: None,
            error_type: None,
        }));
        let recoverable_fn_id = table.intern(LoweredType::Routine(LoweredRoutineType {
            params: vec![],
            return_type: Some(int_id),
            error_type: Some(str_id),
        }));

        assert_eq!(
            render_rust_type(&table, plain_fn_id),
            Ok("fn(rt::FolInt, rt_model::FolStr) -> rt::FolBool".to_string())
        );
        assert_eq!(
            render_rust_type(&table, void_fn_id),
            Ok("fn(rt::FolInt)".to_string())
        );
        assert_eq!(
            render_rust_type(&table, recoverable_fn_id),
            Ok("fn() -> rt::FolRecover<rt::FolInt, rt_model::FolStr>".to_string())
        );
    }
}
