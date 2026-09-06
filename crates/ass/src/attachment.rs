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
fn encode(bytes: &[u8], newline: &str) -> String {
    let mut out = String::new();
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
    out
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
    let mut section = format!("[Fonts]{newline}");
    let mut names = std::collections::BTreeSet::new();
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
        section.push_str(&format!(
            "fontname: {}{newline}{}{newline}{newline}",
            font.name,
            encode(&font.data, newline)
        ));
    }
    Ok(format!("{}{}{}", &text[..offset], section, &text[offset..]))
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
