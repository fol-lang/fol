#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IntrinsicId(usize);

impl IntrinsicId {
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IntrinsicCategory {
    Comparison,
    Boolean,
    Conversion,
    Query,
    Diagnostic,
    Recoverable,
    Memory,
    Pointer,
    Arithmetic,
    Bitwise,
    Introspection,
    NativeAbi,
}

impl IntrinsicCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Comparison => "comparison",
            Self::Boolean => "boolean",
            Self::Conversion => "conversion",
            Self::Query => "query",
            Self::Diagnostic => "diagnostic",
            Self::Recoverable => "recoverable",
            Self::Memory => "memory",
            Self::Pointer => "pointer",
            Self::Arithmetic => "arithmetic",
            Self::Bitwise => "bitwise",
            Self::Introspection => "introspection",
            Self::NativeAbi => "native-abi",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IntrinsicSurface {
    DotRootCall,
    KeywordCall,
    Postfix,
    OperatorAlias,
}

impl IntrinsicSurface {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DotRootCall => "dot-root-call",
            Self::KeywordCall => "keyword-call",
            Self::Postfix => "postfix",
            Self::OperatorAlias => "operator-alias",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IntrinsicAvailability {
    V1,
    V2,
    V3,
    V4,
}

impl IntrinsicAvailability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "V1",
            Self::V2 => "V2",
            Self::V3 => "V3",
            Self::V4 => "V4",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IntrinsicRoadmap {
    CurrentV1,
    LikelyV1x,
    V2,
    V3,
    V4,
    CoreStdInstead,
}

impl IntrinsicRoadmap {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CurrentV1 => "current-v1",
            Self::LikelyV1x => "likely-v1.x",
            Self::V2 => "v2",
            Self::V3 => "v3",
            Self::V4 => "v4",
            Self::CoreStdInstead => "core/std",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IntrinsicBackendRole {
    PureOp,
    ControlEffect,
    RuntimeHook,
    TargetHelper,
}

impl IntrinsicBackendRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PureOp => "pure-op",
            Self::ControlEffect => "control-effect",
            Self::RuntimeHook => "runtime-hook",
            Self::TargetHelper => "target-helper",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IntrinsicStatus {
    Implemented,
    Unsupported,
    Reserved,
}

impl IntrinsicStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Implemented => "implemented",
            Self::Unsupported => "unsupported",
            Self::Reserved => "reserved",
        }
    }
}
