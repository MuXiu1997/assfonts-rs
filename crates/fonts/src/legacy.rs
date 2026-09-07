//! Adapt a legacy Microsoft PRC format-2 cmap without changing source bytes.
//! Only the selected face is rebuilt, with an additional Microsoft Unicode
//! format-12 cmap. Glyph IDs, outlines, names and layout tables are unchanged
//! here; the ordinary HarfBuzz pipeline subsequently subsets and renumbers.

use crate::cp936;
use assfonts_core::{Error, Result};
use ttf_parser::{Face, GlyphId, RawFace, Tag};

fn invalid(message: &str) -> Error {
    Error::Font(format!("legacy PRC cmap: {message}"))
}

fn unsupported() -> Error {
    Error::Unsupported("font has no Unicode cmap; only Microsoft PRC/CP936 format 2 is supported for legacy conversion".into())
}

fn u16_at(data: &[u8], at: usize) -> Result<u16> {
    let bytes = data
        .get(
            at..at
                .checked_add(2)
                .ok_or_else(|| invalid("offset overflow"))?,
        )
        .ok_or_else(|| invalid("truncated table"))?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], at: usize) -> Result<u32> {
    let bytes = data
        .get(
            at..at
                .checked_add(4)
                .ok_or_else(|| invalid("offset overflow"))?,
        )
        .ok_or_else(|| invalid("truncated table"))?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[derive(Clone, Copy)]
pub(crate) struct PrcCmap<'a> {
    data: &'a [u8],
    glyph_count: u16,
}

impl<'a> PrcCmap<'a> {
    /// Leave fonts with a usable Unicode cmap on the exact existing path.
    /// Otherwise follow libass's first-Microsoft-cmap preference, not an
    /// arbitrary later PRC subtable in a mixed legacy font.
    pub(crate) fn from_face(face: &Face<'a>) -> Result<Option<Self>> {
        if face
            .tables()
            .cmap
            .is_some_and(|cmap| cmap.subtables.into_iter().any(|s| s.is_unicode()))
        {
            return Ok(None);
        }
        let cmap = face
            .raw_face()
            .table(Tag::from_bytes(b"cmap"))
            .ok_or_else(unsupported)?;
        if u16_at(cmap, 0)? != 0 {
            return Err(invalid("unsupported cmap version"));
        }
        let count = usize::from(u16_at(cmap, 2)?);
        let header_len = 4 + count * 8;
        if header_len > cmap.len() {
            return Err(invalid("truncated encoding records"));
        }
        let mut first_ms = None;
        for i in 0..count {
            let at = 4 + i * 8;
            let platform = u16_at(cmap, at)?;
            let encoding = u16_at(cmap, at + 2)?;
            // Do not turn a malformed Unicode subtable into a legacy fallback.
            if platform == 0 || (platform == 3 && [1, 10].contains(&encoding)) {
                return Err(invalid("unusable Unicode subtable"));
            }
            if platform == 3 && first_ms.is_none() {
                first_ms = Some((encoding, u32_at(cmap, at + 4)? as usize));
            }
        }
        let Some((3, offset)) = first_ms else {
            return Err(unsupported());
        };
        if offset < header_len {
            return Err(invalid("subtable overlaps encoding records"));
        }
        let data = cmap
            .get(offset..)
            .ok_or_else(|| invalid("subtable offset out of bounds"))?;
        if u16_at(data, 0)? != 2 {
            return Err(unsupported());
        }
        Self::parse(data, face.number_of_glyphs()).map(Some)
    }

    fn parse(bytes: &'a [u8], glyph_count: u16) -> Result<Self> {
        let length = usize::from(u16_at(bytes, 2)?);
        let data = bytes
            .get(..length)
            .ok_or_else(|| invalid("subtable length out of bounds"))?;
        if data.len() < 526 || u16_at(data, 0)? != 2 {
            return Err(invalid("truncated format-2 header"));
        }
        let mut max_key = 0;
        for i in 0..256 {
            let key = usize::from(u16_at(data, 6 + i * 2)?);
            if key % 8 != 0 {
                return Err(invalid("subheader key is not a multiple of eight"));
            }
            max_key = max_key.max(key);
        }
        let glyph_array = 518 + max_key + 8;
        if glyph_array > data.len() {
            return Err(invalid("truncated subheaders"));
        }
        for key in (0..=max_key).step_by(8) {
            let at = 518 + key;
            let first = usize::from(u16_at(data, at)?);
            let count = usize::from(u16_at(data, at + 2)?);
            if first + count > 256 {
                return Err(invalid("low-byte range exceeds 255"));
            }
            if count == 0 {
                continue;
            }
            let delta = u16_at(data, at + 4)?;
            let offset = usize::from(u16_at(data, at + 6)?);
            let start = at + 6 + offset;
            if offset == 0
                || offset % 2 != 0
                || start < glyph_array
                || start + count * 2 > data.len()
            {
                return Err(invalid("glyph array range out of bounds"));
            }
            for i in 0..count {
                let raw = u16_at(data, start + i * 2)?;
                // Format 2 applies a signed delta modulo 65536, but never
                // applies it to a zero entry. Negative deltas must wrap.
                if raw != 0 && raw.wrapping_add(delta) >= glyph_count {
                    return Err(invalid("glyph ID exceeds maxp glyph count"));
                }
            }
        }
        Ok(Self { data, glyph_count })
    }

    fn read(&self, at: usize) -> u16 {
        // All offsets used below were bounded by parse().
        u16::from_be_bytes([self.data[at], self.data[at + 1]])
    }

    fn glyph_for_code(&self, code: u16) -> Option<GlyphId> {
        let first_byte = if code < 256 { code } else { code >> 8 };
        let key = usize::from(self.read(6 + usize::from(first_byte) * 2));
        // A high byte whose key is zero is NOT a DBCS lead byte. Treating
        // its low byte as ASCII would invent coverage for missing glyphs.
        if (code < 256) != (key == 0) {
            return None;
        }
        let at = 518 + key;
        let low = code & 255;
        let first = self.read(at);
        let index = low.checked_sub(first)?;
        if index >= self.read(at + 2) {
            return None;
        }
        let glyph_at = at + 6 + usize::from(self.read(at + 6)) + usize::from(index) * 2;
        let raw = self.read(glyph_at);
        if raw == 0 {
            return None;
        }
        let glyph = raw.wrapping_add(self.read(at + 4));
        (glyph != 0 && glyph < self.glyph_count).then_some(GlyphId(glyph))
    }

    pub(crate) fn glyph_index(&self, ch: char) -> Option<GlyphId> {
        self.glyph_for_code(cp936::encode(ch)?)
    }

    fn unicode_mappings(&self) -> impl Iterator<Item = (u32, u16)> + '_ {
        cp936::mappings().filter_map(|(u, code)| {
            self.glyph_for_code(code)
                .map(|glyph| (u32::from(u), glyph.0))
        })
    }
}

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend(value.to_be_bytes());
}
fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend(value.to_be_bytes());
}

fn with_unicode_cmap(original: &[u8], legacy: &PrcCmap<'_>) -> Result<Vec<u8>> {
    let mut groups: Vec<(u32, u32, u16)> = Vec::new();
    for (cp, glyph) in legacy.unicode_mappings() {
        if let Some((start, end, base)) = groups.last_mut() {
            if cp == *end + 1 && u32::from(glyph) == u32::from(*base) + cp - *start {
                *end = cp;
                continue;
            }
        }
        groups.push((cp, cp, glyph));
    }
    let count = u16_at(original, 2)?;
    let new_count = count
        .checked_add(1)
        .ok_or_else(|| invalid("too many encoding records"))?;
    let old_header = 4 + usize::from(count) * 8;
    let mut records = Vec::new();
    for i in 0..usize::from(count) {
        let at = 4 + i * 8;
        let offset = u32_at(original, at + 4)?;
        if (offset as usize) < old_header || offset as usize >= original.len() {
            return Err(invalid("invalid encoding subtable offset"));
        }
        records.push((
            u16_at(original, at)?,
            u16_at(original, at + 2)?,
            offset
                .checked_add(8)
                .ok_or_else(|| invalid("cmap size overflow"))?,
        ));
    }
    let unicode_offset = u32::try_from(original.len())
        .ok()
        .and_then(|n| n.checked_add(8))
        .ok_or_else(|| invalid("cmap size overflow"))?;
    records.push((3, 10, unicode_offset));
    records.sort_by_key(|&(platform, encoding, _)| (platform, encoding));
    let mut output = Vec::new();
    put16(&mut output, 0);
    put16(&mut output, new_count);
    for (platform, encoding, offset) in records {
        put16(&mut output, platform);
        put16(&mut output, encoding);
        put32(&mut output, offset);
    }
    // Preserve the old subtable payload bytes; only record offsets move.
    output.extend_from_slice(&original[old_header..]);
    put16(&mut output, 12);
    put16(&mut output, 0);
    put32(&mut output, 16 + groups.len() as u32 * 12);
    put32(&mut output, 0); // language
    put32(&mut output, groups.len() as u32);
    for (start, end, glyph) in groups {
        put32(&mut output, start);
        put32(&mut output, end);
        put32(&mut output, u32::from(glyph));
    }
    Ok(output)
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

fn rebuild_face(raw: &RawFace<'_>, index: u32, cmap: &[u8]) -> Result<Vec<u8>> {
    let count = raw.table_records.len();
    if count == 0 || count > 4095 {
        return Err(invalid("invalid SFNT table count"));
    }
    let mut records: Vec<_> = raw.table_records.into_iter().collect();
    records.sort_by_key(|record| record.tag);
    if records.windows(2).any(|pair| pair[0].tag == pair[1].tag) {
        return Err(invalid("duplicate SFNT table tag"));
    }
    let face_offset = if raw.data.starts_with(b"ttcf") {
        let offset_at = 12usize
            .checked_add(
                (index as usize)
                    .checked_mul(4)
                    .ok_or_else(|| invalid("face offset overflow"))?,
            )
            .ok_or_else(|| invalid("face offset overflow"))?;
        u32_at(raw.data, offset_at)? as usize
    } else {
        0
    };
    let mut output = Vec::new();
    put32(&mut output, u32_at(raw.data, face_offset)?);
    let selector = count.ilog2() as u16;
    let search = (1u16 << selector) * 16;
    for value in [count, search, selector, count * 16 - search] {
        put16(&mut output, value);
    }
    output.resize(12 + usize::from(count) * 16, 0);
    let mut head = None;
    for (i, record) in records.iter().enumerate() {
        let original = raw
            .data
            .get(
                record.offset as usize
                    ..(record.offset as usize)
                        .checked_add(record.length as usize)
                        .ok_or_else(|| invalid("table range overflow"))?,
            )
            .ok_or_else(|| invalid("SFNT table out of bounds"))?;
        let table = if record.tag == Tag::from_bytes(b"cmap") {
            cmap
        } else {
            original
        };
        let offset = output.len();
        let length = u32::try_from(table.len()).map_err(|_| invalid("SFNT table too large"))?;
        let end = offset
            .checked_add(table.len())
            .and_then(|n| n.checked_add(3))
            .map(|n| n & !3)
            .filter(|&n| u32::try_from(n).is_ok())
            .ok_or_else(|| invalid("SFNT size overflow"))?;
        output.extend_from_slice(table);
        output.resize(end, 0);
        if record.tag == Tag::from_bytes(b"head") {
            if table.len() < 12 {
                return Err(invalid("truncated head table"));
            }
            head = Some(offset);
            output[offset + 8..offset + 12].fill(0);
        }
        let sum = checksum(&output[offset..offset + table.len()]);
        let at = 12 + i * 16;
        output[at..at + 4].copy_from_slice(&record.tag.0.to_be_bytes());
        output[at + 4..at + 8].copy_from_slice(&sum.to_be_bytes());
        output[at + 8..at + 12].copy_from_slice(&(offset as u32).to_be_bytes());
        output[at + 12..at + 16].copy_from_slice(&length.to_be_bytes());
    }
    let head = head.ok_or_else(|| invalid("missing head table"))?;
    let adjustment = 0xb1b0afbau32.wrapping_sub(checksum(&output));
    output[head + 8..head + 12].copy_from_slice(&adjustment.to_be_bytes());
    Ok(output)
}

/// Returns an owned standalone face with an additional Unicode cmap for a
/// supported CP936-only font. The returned face index is always zero.
/// Unicode fonts return None and must use their original bytes/index unchanged.
/// Never modifies the input or changes original source identity/report hashes.
pub fn prepare_unicode_face(bytes: &[u8], index: u32) -> Result<Option<Vec<u8>>> {
    let face = Face::parse(bytes, index).map_err(|e| Error::Font(e.to_string()))?;
    let Some(legacy) = PrcCmap::from_face(&face)? else {
        return Ok(None);
    };
    let raw = face.raw_face();
    let cmap = raw
        .table(Tag::from_bytes(b"cmap"))
        .ok_or_else(unsupported)?;
    let cmap = with_unicode_cmap(cmap, &legacy)?;
    let output = rebuild_face(raw, index, &cmap)?;
    // Keep generation bugs as errors, not a corrupt input to the native bridge.
    let parsed = Face::parse(&output, 0).map_err(|e| invalid(&format!("generated SFNT: {e}")))?;
    for (cp, glyph) in legacy.unicode_mappings() {
        if parsed.glyph_index(char::from_u32(cp).unwrap()) != Some(GlyphId(glyph)) {
            return Err(invalid(
                "generated Unicode cmap does not preserve glyph IDs",
            ));
        }
    }
    Ok(Some(output))
}
