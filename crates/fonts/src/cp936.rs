//! Pinned Microsoft CP936, excluding best-fit/replacement conversions.
//! This is data lookup only: native and WASM never call a system codec.
//! Provenance, license and reproduction are in ../data/README.md.

const TABLE: &[u8] = include_bytes!("../data/cp936.bin");

pub(crate) fn encode(ch: char) -> Option<u16> {
    let key = u16::try_from(ch as u32).ok()?;
    let mut left = 0;
    let mut right = TABLE.len() / 4;
    while left < right {
        let mid = left + (right - left) / 2;
        let at = mid * 4;
        let codepoint = u16::from_be_bytes([TABLE[at], TABLE[at + 1]]);
        match key.cmp(&codepoint) {
            std::cmp::Ordering::Less => right = mid,
            std::cmp::Ordering::Greater => left = mid + 1,
            std::cmp::Ordering::Equal => {
                return Some(u16::from_be_bytes([TABLE[at + 2], TABLE[at + 3]]));
            }
        }
    }
    None
}

pub(crate) fn mappings() -> impl Iterator<Item = (u16, u16)> {
    TABLE.chunks_exact(4).map(|entry| {
        (
            u16::from_be_bytes([entry[0], entry[1]]),
            u16::from_be_bytes([entry[2], entry[3]]),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn exact_cp936_has_no_best_fit_or_gb18030_reassignment() {
        for (ch, expected) in [
            ('A', 0x41),
            ('中', 0xd6d0),
            ('「', 0xa1b8),
            ('€', 0x80),
            ('\u{e7c7}', 0xa8bc),
            ('\u{e5e5}', 0xa3a0),
            ('\u{f8f5}', 0xff),
        ] {
            assert_eq!(encode(ch), Some(expected));
        }
        // GB18030 assigns A8BC to U+1E3F; CP936 keeps its original PUA.
        // Neither transliteration nor replacement with '?' is permissible.
        for ch in ['\u{1e3f}', '\u{378}', '\u{2212}', '😀', '\u{10ffff}'] {
            assert_eq!(encode(ch), None);
        }
    }

    #[test]
    fn generated_map_is_sorted_bijective_and_covers_the_full_bmp_probe() {
        assert_eq!(TABLE.len(), 24070 * 4);
        assert_eq!(
            assfonts_core::sha256(TABLE),
            "95bee7f7f9af99c8610e8e70a8ada4de9d80303a202414491e3025350fb8dcc9"
        );
        let pairs: Vec<_> = mappings().collect();
        assert!(pairs.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(
            pairs.iter().map(|p| p.1).collect::<BTreeSet<_>>().len(),
            pairs.len()
        );
        for (u, code) in &pairs {
            assert_eq!(encode(char::from_u32(u32::from(*u)).unwrap()), Some(*code));
        }
        let present: BTreeSet<_> = pairs.iter().map(|p| p.0).collect();
        for u in 0..=0xffff {
            if let Some(ch) = char::from_u32(u) {
                assert_eq!(encode(ch).is_some(), present.contains(&(u as u16)));
            }
        }
    }
}
