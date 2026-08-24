//! `fol tool abi inspect` and `fol tool abi check`.
//!
//! Both read written `.folabi.json` manifests and nothing else. That is the
//! point: an installed prefix or an extracted release archive is exactly where
//! a consumer needs to ask what a library's C surface is, and neither has a
//! source tree to compile.
//!
//! Reading goes through `AbiManifest::parse`, which recomputes both recorded
//! fingerprints. A hand-edited manifest is refused rather than reported on --
//! comparing one would answer a question about a file nobody generated.

use std::path::{Path, PathBuf};

use crate::{
    cli::{AbiCheckCommand, AbiInspectCommand},
    FrontendCommandResult, FrontendConfig, FrontendError, FrontendErrorKind, FrontendResult,
};

/// Print what one manifest says a library's C surface is.
pub fn abi_inspect_command(
    command: &AbiInspectCommand,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    let path = resolve(&config.working_directory, &command.manifest);
    let manifest = read_manifest(&path)?;
    let surface = &manifest.surface;

    // The report is the point of the command, so it goes in `payload`: human
    // and plain modes print it raw, and JSON carries it verbatim. That is the
    // parity the plan asks for, without a third rendering to keep in step.
    let mut lines = Vec::new();
    let summary = format!(
        "{} exposes {} symbol(s) at ABI {}.{} for {}",
        surface.artifact,
        surface.interface.routines.len(),
        surface.major,
        surface.minor,
        surface.interface.target.rust_target_triple(),
    );

    // One line per symbol, in the manifest's own order, which is sorted by
    // symbol. A reader diffing two inspections gets a stable diff.
    for routine in &surface.interface.routines {
        let parameters = routine
            .parameters
            .iter()
            .map(|parameter| {
                format!(
                    "{} {}",
                    c_type(&surface.interface.types, parameter.type_id),
                    parameter.name
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let error = match &routine.error {
            fol_abi::AbiErrorContract::Infallible => "infallible".to_string(),
            fol_abi::AbiErrorContract::Recoverable { error_type } => format!(
                "recoverable({})",
                c_type(&surface.interface.types, *error_type)
            ),
        };
        lines.push(format!(
            "{} <- {} ({}) -> {} [{}]",
            routine.symbol,
            routine.fol_path,
            parameters,
            c_type(&surface.interface.types, routine.result),
            error,
        ));
    }

    lines.push(format!(
        "interface-fingerprint {}",
        manifest.interface_fingerprint()
    ));
    lines.push(format!(
        "build-fingerprint {}",
        manifest.build_fingerprint()
    ));

    let mut result = FrontendCommandResult::new("tool abi inspect", summary.clone());
    // The summary leads the payload for the same reason it does in `check`:
    // a payload replaces the status envelope in human and plain modes, so a
    // line that only lives in the summary is invisible in a terminal.
    let mut payload = vec![summary];
    payload.extend(lines);
    result.payload = Some(payload.join("\n"));
    Ok(result)
}

/// Compare a candidate manifest against a checked-in baseline.
///
/// Fails on a breaking change and on a target mismatch. The two are reported
/// differently because they mean different things: a break says a symbol
/// changed, and a mismatch says the baseline is not evidence for this target at
/// all -- sending a reader looking for a source change that does not exist is
/// worse than saying nothing.
pub fn abi_check_command(
    command: &AbiCheckCommand,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    let baseline_path = resolve(&config.working_directory, &command.baseline);
    let candidate_path = resolve(&config.working_directory, &command.candidate);
    let baseline = read_manifest(&baseline_path)?;
    let candidate = read_manifest(&candidate_path)?;

    let verdict = fol_abi::compare_surfaces(&baseline.surface, &candidate.surface);
    let build_only = baseline.interface_fingerprint() == candidate.interface_fingerprint()
        && baseline.build_fingerprint() != candidate.build_fingerprint();

    let summary = match verdict {
        fol_abi::AbiCompatibility::Identical if build_only => {
            "the public surface is unchanged; only the build fingerprint moved".to_string()
        }
        fol_abi::AbiCompatibility::Identical => "the public surface is unchanged".to_string(),
        fol_abi::AbiCompatibility::MinorCompatible => {
            "compatible: existing symbols are unchanged and the candidate only adds".to_string()
        }
        fol_abi::AbiCompatibility::TargetMismatch => format!(
            "target mismatch: the baseline describes {} and the candidate {}",
            baseline.surface.interface.target.rust_target_triple(),
            candidate.surface.interface.target.rust_target_triple(),
        ),
        fol_abi::AbiCompatibility::Breaking => {
            "breaking: a public symbol, type, layout, or error rule changed".to_string()
        }
    };

    let mut lines = describe_differences(&baseline, &candidate);
    if build_only {
        lines.push(format!(
            "build-fingerprint {} -> {}",
            baseline.build_fingerprint(),
            candidate.build_fingerprint()
        ));
    }

    match verdict {
        fol_abi::AbiCompatibility::Identical | fol_abi::AbiCompatibility::MinorCompatible => {
            Ok(report(summary, lines))
        }
        fol_abi::AbiCompatibility::Breaking if command.allow_breaking => {
            lines.push(
                "accepted by --allow-breaking; the ABI major must be incremented".to_string(),
            );
            Ok(report(summary, lines))
        }
        fol_abi::AbiCompatibility::Breaking => Err(FrontendError::new(
            FrontendErrorKind::CommandFailed,
            "the candidate breaks the checked-in ABI baseline; increment the ABI major and pass \
             --allow-breaking to accept it deliberately",
        )),
        // Never acceptable with `--allow-breaking`: that flag says "this break
        // is intended", and a target mismatch is not a break to intend.
        fol_abi::AbiCompatibility::TargetMismatch => Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            "the baseline and the candidate describe different targets, so no comparison of \
             their layouts means anything; compare against the baseline for this target",
        )),
    }
}

/// One report, rendered the same way in every output mode.
///
/// The verdict is the first *payload* line, not only the summary: human and
/// plain modes print a payload raw and drop the status envelope, so a verdict
/// left in the summary would be visible in JSON and invisible in a terminal --
/// and the verdict is the whole answer.
fn report(summary: String, lines: Vec<String>) -> FrontendCommandResult {
    let mut result = FrontendCommandResult::new("tool abi check", summary.clone());
    let mut payload = vec![summary];
    payload.extend(lines);
    result.payload = Some(payload.join("\n"));
    result
}

/// Symbols the candidate added or removed, and the ones whose shape moved.
///
/// Reported for every verdict, including the compatible ones: a reader wants to
/// know *what* was added, not only that adding was allowed.
fn describe_differences(
    baseline: &fol_abi::AbiManifest,
    candidate: &fol_abi::AbiManifest,
) -> Vec<String> {
    let mut lines = Vec::new();
    let base: Vec<&str> = baseline
        .surface
        .interface
        .routines
        .iter()
        .map(|routine| routine.symbol.as_str())
        .collect();
    let cand: Vec<&str> = candidate
        .surface
        .interface
        .routines
        .iter()
        .map(|routine| routine.symbol.as_str())
        .collect();

    for symbol in &cand {
        if !base.contains(symbol) {
            lines.push(format!("added {symbol}"));
        }
    }
    for symbol in &base {
        if !cand.contains(symbol) {
            lines.push(format!("removed {symbol}"));
        }
    }
    for routine in &baseline.surface.interface.routines {
        let Some(updated) = candidate
            .surface
            .interface
            .routines
            .iter()
            .find(|other| other.symbol == routine.symbol)
        else {
            continue;
        };
        let before = signature(routine, &baseline.surface.interface.types);
        let after = signature(updated, &candidate.surface.interface.types);
        if before != after {
            lines.push(format!("changed {}: {before} -> {after}", routine.symbol));
        }
    }
    lines
}

fn signature(routine: &fol_abi::ForeignRoutine, types: &fol_abi::AbiTypeTable) -> String {
    let parameters = routine
        .parameters
        .iter()
        .map(|parameter| c_type(types, parameter.type_id))
        .collect::<Vec<_>>()
        .join(", ");
    format!("({parameters}) -> {}", c_type(types, routine.result))
}

/// The C spelling of one type, for a human reading a report.
fn c_type(types: &fol_abi::AbiTypeTable, id: fol_abi::AbiTypeId) -> String {
    match types.get(id) {
        Some(fol_abi::AbiType::Scalar(scalar)) => scalar.c_type(),
        Some(fol_abi::AbiType::Void) => "void".to_string(),
        Some(fol_abi::AbiType::BorrowedString) => "fol_str_view_t".to_string(),
        Some(fol_abi::AbiType::Record { name, .. })
        | Some(fol_abi::AbiType::Entry { name, .. }) => format!("fol_{}_t", name.to_lowercase()),
        Some(fol_abi::AbiType::OpaqueHandle { name }) => format!("{name} *"),
        Some(fol_abi::AbiType::Pointer { .. }) => "void *".to_string(),
        Some(fol_abi::AbiType::BorrowedSlice { .. }) => "fol_slice_t".to_string(),
        // Shown as C declares it -- a function pointer whose first parameter is
        // the context -- because that is what a consumer reading a header sees.
        Some(fol_abi::AbiType::Callback { parameters, result }) => {
            let rendered = std::iter::once("void *".to_string())
                .chain(parameters.iter().map(|id| c_type(types, *id)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} (*)({rendered})", c_type(types, *result))
        }
        None => "?".to_string(),
    }
}

fn resolve(working_directory: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        working_directory.join(candidate)
    }
}

fn read_manifest(path: &Path) -> FrontendResult<fol_abi::AbiManifest> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!("could not read {}: {error}", path.display()),
        )
    })?;
    fol_abi::AbiManifest::parse(&text).map_err(|error| {
        FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!("{}: {error}", path.display()),
        )
    })
}
