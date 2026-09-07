// Independent PRC cmap fixture over vendored OFL Roboto abc (glyphs 1/2/3).
function checksum(bytes: Uint8Array): number {
  let sum = 0
  for (let i = 0; i < bytes.length; i += 4) {
    sum = (sum + (bytes[i] ?? 0) * 0x1000000 + ((bytes[i + 1] ?? 0) << 16) +
      ((bytes[i + 2] ?? 0) << 8) + (bytes[i + 3] ?? 0)) >>> 0
  }
  return sum
}

function prcCmap(): Uint8Array<ArrayBuffer> {
  const entries = [[0x20, 1], [0x40, 3], [0x41, 1], [0x80, 3], [0xff, 3],
    [0xa1b8, 1], [0xa1b9, 1], [0xa8bc, 3], [0xd6d0, 2], [0xd6d1, 0]]
  const rows = Map.groupBy(entries, ([code]) => code >> 8)
  const payloads = Array.from(rows, ([lead, row]) => {
    const values = new Map(row.map(([code, glyph]) => [code & 255, glyph]))
    const first = Math.min(...values.keys()), last = Math.max(...values.keys())
    return { lead, first, glyphs: Array.from({ length: last - first + 1 }, (_, i) => values.get(first + i) ?? 0) }
  })
  const header = 518 + rows.size * 8
  const length = header + payloads.reduce((sum, row) => sum + row.glyphs.length * 2, 0)
  const bytes = new Uint8Array(12 + length), view = new DataView(bytes.buffer)
  view.setUint16(2, 1); view.setUint16(4, 3); view.setUint16(6, 3); view.setUint32(8, 12)
  view.setUint16(12, 2); view.setUint16(14, length)
  let offset = 12 + header
  payloads.forEach(({ lead, first, glyphs }, index) => {
    view.setUint16(12 + 6 + lead * 2, index * 8)
    const at = 12 + 518 + index * 8
    view.setUint16(at, first); view.setUint16(at + 2, glyphs.length)
    view.setUint16(at + 6, offset - at - 6)
    for (const glyph of glyphs) { view.setUint16(offset, glyph); offset += 2 }
  })
  return bytes
}

export function makeLegacyFont(original: Uint8Array): Uint8Array<ArrayBuffer> {
  const view = new DataView(original.buffer, original.byteOffset, original.byteLength)
  const count = view.getUint16(4)
  const tables = Array.from({ length: count }, (_, i) => {
    const at = 12 + i * 16, tag = view.getUint32(at)
    const off = view.getUint32(at + 8), length = view.getUint32(at + 12)
    const data = tag === 0x636d6170 ? prcCmap() : original.slice(off, off + length)
    if (tag === 0x68656164) data.fill(0, 8, 12)
    return { tag, data }
  }).sort((a, b) => a.tag - b.tag)
  const bytes = new Uint8Array(12 + count * 16 + tables.reduce((sum, t) => sum + Math.ceil(t.data.length / 4) * 4, 0))
  const out = new DataView(bytes.buffer)
  out.setUint32(0, view.getUint32(0)); out.setUint16(4, count)
  const selector = Math.floor(Math.log2(count)), search = 16 * 2 ** selector
  out.setUint16(6, search); out.setUint16(8, selector); out.setUint16(10, count * 16 - search)
  let offset = 12 + count * 16, head = 0
  tables.forEach(({ tag, data }, i) => {
    const at = 12 + i * 16
    out.setUint32(at, tag); out.setUint32(at + 4, checksum(data))
    out.setUint32(at + 8, offset); out.setUint32(at + 12, data.length)
    bytes.set(data, offset)
    if (tag === 0x68656164) head = offset
    offset += Math.ceil(data.length / 4) * 4
  })
  if (!head) throw new Error('Missing head table')
  out.setUint32(head + 8, (0xb1b0afba - checksum(bytes)) >>> 0)
  return bytes
}
