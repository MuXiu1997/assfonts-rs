use assfonts_core::{Attachment, Error, Result};

// This encoder is adapted from the Aegisub UUEncode implementation.
//
// Copyright (c) 2013, Thomas Goyne <plorkyeran@aegisub.org>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//
// Aegisub Project: https://github.com/Aegisub/Aegisub

// ASS uses 6-bit values offset by 33, no length prefix, no padding, 80 columns.
fn encode_into(out: &mut String, bytes: &[u8], newline: &str) {
    let mut column = 0;
    for chunk in bytes.chunks(3) {
        let mut src = [0u8; 3];
        src[..chunk.len()].copy_from_slice(chunk);
        let values = [
            src[0] >> 2,
            ((src[0] & 3) << 4) | (src[1] >> 4),
            ((src[1] & 15) << 2) | (src[2] >> 6),
            src[2] & 63,
        ];
        for value in &values[..chunk.len() + 1] {
            if column == 80 {
                out.push_str(newline);
                column = 0;
            }
            out.push(char::from(value + 33));
            column += 1;
        }
    }
}

fn encoded_len(bytes: usize, newline_len: usize) -> Option<usize> {
    let chars = (bytes / 3).checked_mul(4)?.checked_add(match bytes % 3 {
        0 => 0,
        n => n + 1,
    })?;
    chars.checked_add((chars.saturating_sub(1) / 80).checked_mul(newline_len)?)
}

pub(super) fn embed(text: &str, fonts: &[Attachment]) -> Result<String> {
    if fonts.is_empty() {
        return Ok(text.into());
    }
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.trim().eq_ignore_ascii_case("[Events]") {
            break;
        }
        offset += line.len();
    }
    if offset == text.len() {
        return Err(Error::Subtitle("missing [Events] insertion point".into()));
    }
    let mut names = std::collections::BTreeSet::new();
    let mut size = text.len().checked_add("[Fonts]".len() + newline.len());
    for font in fonts {
        if font.data.is_empty()
            || font.name.is_empty()
            || !font
                .name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
            || !names.insert(&font.name)
        {
            return Err(Error::Subtitle(
                "empty/invalid/duplicate font attachment".into(),
            ));
        }
        size = size
            .and_then(|n| n.checked_add("fontname: ".len()))
            .and_then(|n| n.checked_add(font.name.len()))
            .and_then(|n| n.checked_add(newline.len() * 3))
            .and_then(|n| n.checked_add(encoded_len(font.data.len(), newline.len())?));
    }
    let size = size.ok_or_else(|| Error::Subtitle("font attachments are too large".into()))?;
    // Append directly to the final subtitle. Avoid an encoded temporary per
    // font, a formatted copy of it, and a second copy of the whole Fonts section.
    let mut output = String::with_capacity(size);
    output.push_str(&text[..offset]);
    output.push_str("[Fonts]");
    output.push_str(newline);
    for font in fonts {
        output.push_str("fontname: ");
        output.push_str(&font.name);
        output.push_str(newline);
        encode_into(&mut output, &font.data, newline);
        output.push_str(newline);
        output.push_str(newline);
    }
    output.push_str(&text[offset..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn encode(bytes: &[u8], newline: &str) -> String {
        let mut out = String::new();
        encode_into(&mut out, bytes, newline);
        out
    }
    #[test]
    fn ass_known_vectors_and_line_boundary() {
        assert_eq!(encode(b"Cat", "\n"), "1W&U");
        assert_eq!(encode(&[0], "\n"), "!!");
        assert_eq!(encode(&[255, 255], "\n"), "``]");
        assert_eq!(
            encode(&[0; 61], "\r\n"),
            format!("{}\r\n!!", "!".repeat(80))
        );
    }

    #[test]
    fn encoded_sizes_cover_partial_groups_and_both_line_endings() {
        for newline in ["\n", "\r\n"] {
            for size in 0..1024 {
                assert_eq!(
                    encoded_len(size, newline.len()),
                    Some(encode(&vec![0xff; size], newline).len())
                );
            }
        }
        assert_eq!(encoded_len(usize::MAX, 2), None);
    }
}
