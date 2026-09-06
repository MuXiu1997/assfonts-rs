//! Font metadata and deterministic matching over caller-supplied bytes.
#![forbid(unsafe_code)]

use assfonts_core::{sha256, Error, FontFace, FontRequest, FontResolver, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

struct Candidate {
    face: FontFace,
    weight: u16,
    italic: bool,
}

#[derive(Default)]
pub struct FontCatalog {
    faces: Vec<Candidate>,
    names: BTreeMap<String, Vec<usize>>,
    identities: BTreeSet<String>,
}

impl FontCatalog {
    /// Adds all faces of a TTF/OTF/TTC/OTC. Byte-identical sources are deduplicated.
    /// Parsing is transactional: a malformed collection contributes no faces.
    pub fn add(&mut self, source: impl Into<String>, bytes: impl Into<Arc<[u8]>>) -> Result<()> {
        let source = source.into();
        let data = bytes.into();
        let identity = sha256(&data);
        if self.identities.contains(&identity) {
            return Ok(());
        }
        let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        if count == 0 {
            return Err(Error::Font(format!("{source}: empty collection")));
        }
        let mut pending = Vec::new();
        for index in 0..count {
            let parsed = ttf_parser::Face::parse(&data, index)
                .map_err(|e| Error::Font(format!("{source} face {index}: {e}")))?;
            let names: BTreeSet<_> = parsed
                .names()
                .into_iter()
                .filter(|n| [1, 4, 6, 16, 21].contains(&n.name_id))
                .filter_map(|n| n.to_string())
                .map(|n| normalize(&n))
                .filter(|n| !n.is_empty())
                .collect();
            if names.is_empty() {
                return Err(Error::Font(format!(
                    "{source} face {index}: no supported font names"
                )));
            }
            pending.push((
                Candidate {
                    face: FontFace {
                        source: source.clone(),
                        data: data.clone(),
                        index,
                    },
                    weight: parsed.weight().to_number(),
                    italic: parsed.is_italic(),
                },
                names,
            ));
        }
        for (candidate, names) in pending {
            let index = self.faces.len();
            for name in names {
                self.names.entry(name).or_default().push(index);
            }
            self.faces.push(candidate);
        }
        self.identities.insert(identity);
        Ok(())
    }
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }
}

fn normalize(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Default-ignorable code points affect shaping but need not have cmap glyphs.
/// They are still passed through to the subsetter's Unicode set.
fn default_ignorable(c: char) -> bool {
    matches!(c as u32, 0x00ad | 0x034f | 0x061c | 0x115f..=0x1160 | 0x17b4..=0x17b5
        | 0x180b..=0x180f | 0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x206f
        | 0x3164 | 0xfe00..=0xfe0f | 0xfeff | 0xffa0 | 0xfff0..=0xfff8
        | 0x1bca0..=0x1bca3 | 0x1d173..=0x1d17a | 0xe0000..=0xe0fff)
}

pub fn verify_coverage(bytes: &[u8], index: u32, characters: &BTreeSet<char>) -> Result<()> {
    let face = ttf_parser::Face::parse(bytes, index).map_err(|e| Error::Font(e.to_string()))?;
    let missing: Vec<_> = characters
        .iter()
        .filter(|&&c| !default_ignorable(c) && face.glyph_index(c).is_none_or(|g| g.0 == 0))
        .take(16)
        .map(|&c| format!("U+{:04X} ({c})", c as u32))
        .collect();
    if !missing.is_empty() {
        return Err(Error::Font(format!(
            "missing glyphs: {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

impl FontResolver for FontCatalog {
    fn resolve(&self, request: &FontRequest, characters: &BTreeSet<char>) -> Result<FontFace> {
        let candidates = self.names.get(&normalize(&request.family)).ok_or_else(|| {
            Error::Font(format!(
                "font {:?} not found in supplied sources",
                request.family
            ))
        })?;
        let mut ranked: Vec<_> = candidates
            .iter()
            .map(|&i| {
                let c = &self.faces[i];
                (
                    u32::from(c.weight.abs_diff(request.weight))
                        + if c.italic != request.italic { 1000 } else { 0 },
                    i,
                )
            })
            .collect();
        ranked.sort_unstable();
        let (score, index) = ranked[0];
        if ranked.get(1).is_some_and(|x| x.0 == score) {
            let labels: Vec<_> = ranked
                .iter()
                .take_while(|x| x.0 == score)
                .map(|x| {
                    let f = &self.faces[x.1].face;
                    format!("{} face {}", f.source, f.index)
                })
                .collect();
            return Err(Error::Font(format!(
                "ambiguous {:?} weight {} italic {}: {}",
                request.family,
                request.weight,
                request.italic,
                labels.join("; ")
            )));
        }
        let face = &self.faces[index].face;
        verify_coverage(&face.data, face.index, characters).map_err(|e| {
            Error::Font(format!(
                "{:?} -> {} face {}: {e}",
                request.family, face.source, face.index
            ))
        })?;
        Ok(face.clone())
    }
}
