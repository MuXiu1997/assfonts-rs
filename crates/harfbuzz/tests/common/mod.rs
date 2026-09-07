use std::collections::BTreeMap;

pub const BASE: &[u8] =
    include_bytes!("../../../../vendor/harfbuzz/test/api/fonts/OpenSans-Regular.ttf");
pub fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes(b[at..at + 2].try_into().unwrap())
}
fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}
pub fn put16(b: &mut Vec<u8>, n: u16) {
    b.extend(n.to_be_bytes());
}
pub fn put32(b: &mut Vec<u8>, n: u32) {
    b.extend(n.to_be_bytes());
}
fn checksum(b: &[u8]) -> u32 {
    b.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

// Construct a synthetic SFNT around the vendored OFL fixture, without a new
// font binary. This also exercises normal table checksums and head adjustment.
pub fn with_table(tag: [u8; 4], table: &[u8]) -> Vec<u8> {
    let mut tables = BTreeMap::new();
    for i in 0..usize::from(u16_at(BASE, 4)) {
        let at = 12 + i * 16;
        let tag: [u8; 4] = BASE[at..at + 4].try_into().unwrap();
        let off = u32_at(BASE, at + 8) as usize;
        let len = u32_at(BASE, at + 12) as usize;
        let mut data = BASE[off..off + len].to_vec();
        if tag == *b"head" {
            data[8..12].fill(0);
        }
        tables.insert(tag, data);
    }
    tables.insert(tag, table.to_vec());
    let n = tables.len() as u16;
    let selector = n.ilog2() as u16;
    let search = (1 << selector) * 16;
    let mut output = vec![];
    put32(&mut output, 0x10000);
    for value in [n, search, selector, n * 16 - search] {
        put16(&mut output, value);
    }
    output.resize(12 + tables.len() * 16, 0);
    let mut head = 0;
    for (i, (tag, data)) in tables.iter().enumerate() {
        let off = output.len();
        if tag == b"head" {
            head = off;
        }
        let at = 12 + i * 16;
        output[at..at + 4].copy_from_slice(tag);
        output[at + 4..at + 8].copy_from_slice(&checksum(data).to_be_bytes());
        output[at + 8..at + 12].copy_from_slice(&(off as u32).to_be_bytes());
        output[at + 12..at + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend(data);
        output.resize(output.len().next_multiple_of(4), 0);
    }
    let adjust = 0xb1b0afbau32.wrapping_sub(checksum(&output));
    output[head + 8..head + 12].copy_from_slice(&adjust.to_be_bytes());
    output
}
