use gerc::{GenerationBundle, RustAbi, RustItem, RustScalar, RustTypeKind};

pub(crate) const H7_RAW_CRATE_NAME: &str = "fol_h7_raw";
pub(crate) const H7_ANCHOR_CRATE_NAME: &str = "fol_h7_anchor";
pub(crate) const H7_ANCHOR_FUNCTION_NAME: &str = "fol_h7_read_provider";

/// Narrow FOL-owned safe wrapper used by the mandatory H7 link/read smoke.
///
/// It is intentionally not a general C-to-Rust emitter. The raw declaration
/// and identifier come from GERC; this wrapper accepts exactly one measured,
/// no-argument C function returning a 32-bit C `int`, so the final executable
/// must retain and call a real provider symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct H7InteropAnchor {
    source: Vec<u8>,
}

impl H7InteropAnchor {
    pub fn source(&self) -> &[u8] {
        &self.source
    }
}

pub(crate) fn h7_c_int_function_anchor(
    bundle: &GenerationBundle,
) -> Result<H7InteropAnchor, H7InteropAnchorError> {
    let [RustItem::Function(function)] = bundle.projection().items() else {
        return Err(H7InteropAnchorError::ExpectedSingleFunction);
    };
    if function.abi() != RustAbi::C {
        return Err(H7InteropAnchorError::ExpectedCAbi);
    }
    if !function.parameters().is_empty() || function.variadic() {
        return Err(H7InteropAnchorError::ExpectedNoArguments);
    }
    if !matches!(
        function.return_type().kind(),
        RustTypeKind::Scalar(RustScalar::CInt {
            storage_bits: 32,
            alignment_bits: 32,
        })
    ) {
        return Err(H7InteropAnchorError::ExpectedCInt);
    }

    let rust_name = function.rust_name().as_str();
    let source = format!(
        "#![no_std]\n\
         #[inline(never)]\n\
         pub fn {H7_ANCHOR_FUNCTION_NAME}() -> i32 {{\n\
             unsafe {{ {H7_RAW_CRATE_NAME}::{rust_name}() as i32 }}\n\
         }}\n"
    )
    .into_bytes();
    Ok(H7InteropAnchor { source })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H7InteropAnchorError {
    ExpectedSingleFunction,
    ExpectedCAbi,
    ExpectedNoArguments,
    ExpectedCInt,
}

impl std::fmt::Display for H7InteropAnchorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ExpectedSingleFunction => {
                "H7 smoke requires exactly one GERC-projected C function"
            }
            Self::ExpectedCAbi => "H7 smoke function must use the C calling convention",
            Self::ExpectedNoArguments => {
                "H7 smoke function must have a non-variadic void parameter list"
            }
            Self::ExpectedCInt => "H7 smoke function must return a measured 32-bit C int",
        })
    }
}

impl std::error::Error for H7InteropAnchorError {}
