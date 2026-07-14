/// Compiler-internal capability tier used after build evaluation.
///
/// Public `fol_model` accepts only `core` and `memo`. `Std` represents the
/// effective hosted tier derived when a `memo` artifact declares the bundled
/// internal `standard` dependency; it is not a legal third public model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypecheckCapabilityModel {
    Core,
    Memo,
    #[default]
    Std,
}

impl TypecheckCapabilityModel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Memo => "memo",
            Self::Std => "std",
        }
    }

    pub fn supports_processor(self) -> bool {
        matches!(self, Self::Std)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TypecheckConfig {
    pub capability_model: TypecheckCapabilityModel,
}
