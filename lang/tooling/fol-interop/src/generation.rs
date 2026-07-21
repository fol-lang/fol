use gerc::{GenerationBundle, GenerationError, GenerationRequest, ItemSelection};
use linc::contract::ValidatedLinkAnalysis;
use parc::contract::CompleteSourcePackage;

/// Project one checked PARC/LINC pair through GERC's sole production intake.
///
/// Selection comes from the exact complete source closure. FOL never creates a
/// parallel declaration model or emits raw Rust declarations itself.
pub(crate) fn generate_raw_bindings(
    source: &CompleteSourcePackage,
    evidence: &ValidatedLinkAnalysis,
) -> Result<GenerationBundle, GenerationError> {
    let selection = ItemSelection::from_complete(source);
    let request = GenerationRequest::try_new(source, evidence, &selection)?;
    gerc::generate(request)
}
