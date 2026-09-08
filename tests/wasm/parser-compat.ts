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
  ]
  for (const [name, input] of variants) {
    if (input === original) throw new Error('Parser fixture did not change: ' + name)
    const result = engine.process(new TextEncoder().encode(input))
    const hashes = result.report.fonts.map((f: { subset_sha256: string }) => f.subset_sha256).sort()
    if (JSON.stringify(hashes) !== JSON.stringify(expectedHashes)) {
      throw new Error('Parser compatibility changed font usage: ' + name)
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
