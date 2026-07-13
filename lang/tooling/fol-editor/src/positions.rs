//! Conversion between FOL's compiler columns and LSP UTF-16 positions.
//!
//! Compiler locations count Unicode scalar values. LSP positions count UTF-16
//! code units unless the server negotiates another encoding. Keep that
//! boundary explicit instead of letting byte, scalar, and wire columns mix.

use crate::{LspPosition, LspRange};

pub(crate) fn utf16_position_to_scalar(text: &str, position: LspPosition) -> Option<LspPosition> {
    let line = source_line(text, position.line)?;
    let character = utf16_character_to_scalar(line, position.character)?;
    Some(LspPosition {
        line: position.line,
        character,
    })
}

/// Convert an LSP request position while retaining the server's historical
/// tolerance for stale/out-of-bounds cursors. Positions past an existing line
/// or on a not-yet-synced line pass through unchanged. A position inside an
/// astral character's surrogate pair remains invalid.
pub(crate) fn utf16_position_to_scalar_tolerant(
    text: &str,
    position: LspPosition,
) -> Option<LspPosition> {
    let Some(line) = source_line(text, position.line) else {
        return Some(position);
    };
    let character = utf16_character_to_scalar(line, position.character).or_else(|| {
        (position.character > line.encode_utf16().count() as u32).then_some(position.character)
    })?;
    Some(LspPosition {
        line: position.line,
        character,
    })
}

pub(crate) fn scalar_position_to_utf16(text: &str, position: LspPosition) -> Option<LspPosition> {
    let line = source_line(text, position.line)?;
    let character = scalar_character_to_utf16(line, position.character)?;
    Some(LspPosition {
        line: position.line,
        character,
    })
}

pub(crate) fn utf16_range_to_scalar_tolerant(text: &str, range: LspRange) -> Option<LspRange> {
    Some(LspRange {
        start: utf16_position_to_scalar_tolerant(text, range.start)?,
        end: utf16_position_to_scalar_tolerant(text, range.end)?,
    })
}

pub(crate) fn scalar_range_to_utf16(text: &str, range: LspRange) -> Option<LspRange> {
    Some(LspRange {
        start: scalar_position_to_utf16(text, range.start)?,
        end: scalar_position_to_utf16(text, range.end)?,
    })
}

pub(crate) fn scalar_position_to_offset(text: &str, position: LspPosition) -> Option<usize> {
    let line_start = line_start_offset(text, position.line)?;
    let line = source_line(text, position.line)?;
    let byte_in_line = scalar_character_to_byte(line, position.character)?;
    Some(line_start + byte_in_line)
}

pub(crate) fn utf16_position_to_offset(text: &str, position: LspPosition) -> Option<usize> {
    scalar_position_to_offset(text, utf16_position_to_scalar(text, position)?)
}

/// Convert delta-encoded semantic tokens whose starts and lengths use compiler
/// scalar columns into the UTF-16 columns required on the LSP wire.
pub(crate) fn scalar_semantic_tokens_to_utf16(text: &str, data: &[u32]) -> Option<Vec<u32>> {
    if !data.len().is_multiple_of(5) {
        return None;
    }

    let mut absolute = Vec::with_capacity(data.len() / 5);
    let mut line = 0_u32;
    let mut start = 0_u32;
    for chunk in data.chunks_exact(5) {
        if chunk[0] == 0 {
            start = start.checked_add(chunk[1])?;
        } else {
            line = line.checked_add(chunk[0])?;
            start = chunk[1];
        }
        let line_text = source_line(text, line)?;
        let utf16_start = scalar_character_to_utf16(line_text, start)?;
        let scalar_end = start.checked_add(chunk[2])?;
        let utf16_end = scalar_character_to_utf16(line_text, scalar_end)?;
        absolute.push((
            line,
            utf16_start,
            utf16_end.checked_sub(utf16_start)?,
            chunk[3],
            chunk[4],
        ));
    }

    let mut converted = Vec::with_capacity(data.len());
    let mut previous_line = 0_u32;
    let mut previous_start = 0_u32;
    for (index, (line, start, length, token_type, modifiers)) in absolute.into_iter().enumerate() {
        let delta_line = if index == 0 {
            line
        } else {
            line.checked_sub(previous_line)?
        };
        let delta_start = if index == 0 || delta_line != 0 {
            start
        } else {
            start.checked_sub(previous_start)?
        };
        converted.extend([delta_line, delta_start, length, token_type, modifiers]);
        previous_line = line;
        previous_start = start;
    }
    Some(converted)
}

fn source_line(text: &str, line: u32) -> Option<&str> {
    text.split('\n').nth(line as usize)
}

fn line_start_offset(text: &str, line: u32) -> Option<usize> {
    if line == 0 {
        return Some(0);
    }
    text.match_indices('\n')
        .nth(line.saturating_sub(1) as usize)
        .map(|(offset, _)| offset + 1)
}

fn utf16_character_to_scalar(line: &str, character: u32) -> Option<u32> {
    if character == 0 {
        return Some(0);
    }
    let mut utf16 = 0_u32;
    for (scalar, ch) in line.chars().enumerate() {
        utf16 = utf16.checked_add(ch.len_utf16() as u32)?;
        if utf16 == character {
            return Some(scalar as u32 + 1);
        }
        if utf16 > character {
            return None;
        }
    }
    None
}

fn scalar_character_to_utf16(line: &str, character: u32) -> Option<u32> {
    let mut scalars = 0_u32;
    let mut utf16 = 0_u32;
    for ch in line.chars().take(character as usize) {
        scalars += 1;
        utf16 += ch.len_utf16() as u32;
    }
    (scalars == character).then_some(utf16)
}

fn scalar_character_to_byte(line: &str, character: u32) -> Option<usize> {
    if character == 0 {
        return Some(0);
    }
    line.char_indices()
        .nth(character as usize)
        .map(|(offset, _)| offset)
        .or_else(|| (line.chars().count() == character as usize).then_some(line.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn astral_characters_use_two_utf16_code_units() {
        let text = "a😀b\n🌱";

        assert_eq!(
            utf16_position_to_scalar(
                text,
                LspPosition {
                    line: 0,
                    character: 3,
                },
            ),
            Some(LspPosition {
                line: 0,
                character: 2,
            })
        );
        assert_eq!(
            scalar_position_to_utf16(
                text,
                LspPosition {
                    line: 0,
                    character: 2,
                },
            ),
            Some(LspPosition {
                line: 0,
                character: 3,
            })
        );
        assert_eq!(
            utf16_position_to_offset(
                text,
                LspPosition {
                    line: 0,
                    character: 3,
                },
            ),
            Some("a😀".len())
        );
    }

    #[test]
    fn positions_inside_surrogate_pairs_are_rejected() {
        assert_eq!(
            utf16_position_to_scalar(
                "😀",
                LspPosition {
                    line: 0,
                    character: 1,
                },
            ),
            None
        );
    }

    #[test]
    fn semantic_token_columns_and_lengths_convert_to_utf16() {
        // Scalar token starts at column 2 and covers the astral character plus
        // the following ASCII character (two scalars, three UTF-16 units).
        let converted = scalar_semantic_tokens_to_utf16("x 😀z", &[0, 2, 2, 4, 0]).unwrap();
        assert_eq!(converted, vec![0, 2, 3, 4, 0]);
    }
}
