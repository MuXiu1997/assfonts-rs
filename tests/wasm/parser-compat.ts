import type { MemoryEngine } from './abi.ts'

export async function verifyParserCompatibility(
  engine: MemoryEngine, original: string, expectedHashes: string[], output: string,
) {
  const style = original.split('\n').find(line => line.startsWith('Style: Default,'))
  if (!style) throw new Error('Missing Default fixture style')
  const variants = [
    ['star-style', original.replaceAll(',Default,,', ',*Default,,')],
    ['color-args', original.replace('\\1c&H000000&\\3c&HFFFFFF&', '\\1c000000\\3cFFFFFF')],
    ['empty-override', original.replaceAll('\\fn', '\\\\fn')],
    ['missing-reset', original.replace('\\rDefault', '\\rDefault\\r0')],
    ['duplicate-style', original.replace(style, style.replace('Noto Sans,', 'Noto Sans SC,') + '\n' + style)],
    ['tab-as-space', original.replace('office ffi', 'office\tffi')],
    ['ignored-preamble', '; [Script Info]\n==== ignored preamble ====\nWrapStyle: 0\n' + original],
    ['del-character', original.replace('office', 'office\u007f')],
    ['tagless-transform', original.replace('office', '{\\t(100,6590)}office')],
    ['unterminated-transform', original.replace('\\t(0,900,\\fscx120)', '\\t(0,900,\\fscx120')],
    ['bold-style-fallback', original
      .replace('office', '{\\b0\\b20}office')
      .replace('中文标点', '{\\b1\\b2}中文标点')
      .replace('\\rDefault', '\\rDefault\\b0\\b4')],
  ]
  for (const [name, input] of variants) {
    if (input === original) throw new Error('Parser fixture did not change: ' + name)
    const isDel = name === 'del-character'
    // DEL must reach glyph resolution, including warn mode if this font lacks it.
    if (isDel) engine.setMissingGlyphPolicy('warn')
    let result
    try { result = engine.process(new TextEncoder().encode(input)) }
    finally { if (isDel) engine.setMissingGlyphPolicy('error') }
    const hashes = result.report.fonts.map((f: { subset_sha256: string }) => f.subset_sha256).sort()
    if (!isDel && JSON.stringify(hashes) !== JSON.stringify(expectedHashes)) {
      throw new Error('Parser compatibility changed font usage: ' + name)
    }
    if (isDel && !result.report.fonts.some((f: { characters: string }) => f.characters.includes('\u007f'))) {
      throw new Error('DEL was silently dropped before glyph resolution')
    }
    const preserved = result.subtitle.replace(/\[Fonts\]\n[\s\S]*?(?=\[Events\])/, '')
    if (preserved !== input) throw new Error('Parser compatibility rewrote ASS: ' + name)
    const directory = output + '/parser-' + name
    await Deno.mkdir(directory, { recursive: true })
    await Deno.writeTextFile(directory + '/input.ass', input)
    await Deno.writeTextFile(directory + '/input.assfonts.ass', result.subtitle)
    const [prefix, rest] = result.subtitle.split('[Fonts]\n')
    const [attachments, events] = rest.split('[Events]')
    for (const [source, filename] of [['NotoSans.ttc', 'ttc-only.ass'], ['NotoSansSC-Regular.otf', 'otf-only.ass']]) {
      const font = result.report.fonts.find((f: { source: string }) => f.source === source)
      const entry = attachments.split('fontname: ').slice(1).find((value: string) => value.startsWith(font.attachment + '\n'))
      if (!entry) throw new Error('Missing parser test attachment')
      await Deno.writeTextFile(directory + '/' + filename, prefix + '[Fonts]\nfontname: ' + entry.trim() + '\n\n[Events]' + events)
    }
  }
}
