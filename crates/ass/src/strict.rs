//! Acceptance checks only: never use this tokenization for font analysis.
//! Retain the pre-compatibility name/integer checks, but walk nested transforms.
use ass_core::analysis::events::{parse_override_block, DiagnosticKind};
use assfonts_core::Result;

use crate::{ignored_tag, integer, invalid};

pub(super) fn validate(block: &str, offset: usize) -> Result<()> {
    let mut pending = vec![(block, offset, 0)];
    while let Some((body, start, depth)) = pending.pop() {
        let mut tags = Vec::new();
        let mut diagnostics = Vec::new();
        parse_override_block(body, start, &mut tags, &mut diagnostics);
        if let Some(d) = diagnostics
            .iter()
            .find(|d| d.kind != DiagnosticKind::EmptyOverride || d.span != "\\")
        {
            return Err(invalid(format!(
                "strict syntax at dialogue text byte {}: {:?} ({:?})",
                d.offset, d.span, d.kind
            )));
        }
        for tag in tags {
            let arg = tag.args().trim();
            let fail = |reason: &str| {
                invalid(format!(
                    "strict syntax at dialogue text byte {}: \\{}{}: {reason}",
                    tag.position(),
                    tag.name(),
                    tag.args()
                ))
            };
            match tag.name() {
                "t" => {
                    let inner = arg
                        .strip_prefix('(')
                        .ok_or_else(|| fail("expected transform parentheses"))?;
                    let inner = inner.strip_suffix(')').unwrap_or(inner);
                    if let Some(slash) = inner.find('\\') {
                        // The analyzer counts inner transforms, excluding the
                        // outer event-level t. Keep exactly the same limit.
                        if depth > 64 {
                            return Err(fail("transform nesting exceeds 64"));
                        }
                        let inner = &inner[slash..];
                        let inner_offset =
                            start + (inner.as_ptr() as usize - body.as_ptr() as usize);
                        pending.push((inner, inner_offset, depth + 1));
                    }
                }
                "b" | "i" | "p" | "q" => {
                    if !arg.is_empty() {
                        let value =
                            integer(arg).map_err(|_| fail("expected an exact i32 integer"))?;
                        if tag.name() == "i" && !matches!(value, 0 | 1) {
                            return Err(fail("italic must be 0 or 1"));
                        }
                        if tag.name() == "q" && !(0..=3).contains(&value) {
                            return Err(fail("wrap mode must be 0..3"));
                        }
                    }
                }
                "fn" | "r" | "fe" => (),
                name if ignored_tag(name) => (),
                "blu" | "bklur" => {
                    return Err(fail(
                        "unknown tag; possible misspelling of \\blur (not corrected)",
                    ))
                }
                _ => return Err(fail("unknown tag")),
            }
        }
    }
    Ok(())
}
