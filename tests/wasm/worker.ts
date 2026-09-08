import { loadModule, MemoryEngine } from './abi.ts'
import { makeLegacyFont } from './legacy-font.ts'
import { verifyParserCompatibility } from './parser-compat.ts'

const scope = globalThis as unknown as {
  onmessage: (event: MessageEvent<{ fonts: string; output: string }>) => void
  postMessage(data: unknown): void
}

function require(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

function expectError(call: () => unknown, pattern?: string) {
  try { call() } catch (error) {
    if (pattern) require(String(error).includes(pattern), String(error))
    return
  }
  throw new Error('Expected rejection')
}

scope.onmessage = async ({ data: { fonts, output } }) => {
  let engine: MemoryEngine | undefined
  try {
    const module = await loadModule()
    // A valid font with harmless trailing padding makes a redundant full copy
    // large enough to force growth in a fresh module. Duplicates must reuse the
    // catalog and the input allocation freed by addFont's finally block.
    const padded = new Uint8Array(8 * 1024 * 1024)
    padded.set(await Deno.readFile(new URL('../../vendor/harfbuzz/test/api/fonts/OpenSans-Regular.ttf', import.meta.url)))
    const duplicateProbe = new MemoryEngine(module)
    try {
      duplicateProbe.addFont('padded.ttf', padded)
      const capacity = module.HEAPU8.length
      for (let i = 0; i < 5; i++) {
        require(duplicateProbe.addFont('duplicate.ttf', padded).faces === 1, 'Duplicate added a face')
        require(module.HEAPU8.length === capacity, 'Duplicate font forced an unnecessary full copy')
      }
    } finally { duplicateProbe.close() }
    engine = new MemoryEngine(module)
    const active = engine
    const input = await Deno.readFile(new URL('../fixtures/ci-fonts.ass', import.meta.url))
    expectError(() => active.process(new Uint8Array([255])), 'utf-8')
    expectError(() => active.process(input)) // No system-font fallback.
    expectError(() => active.addFont('invalid.ttf', new Uint8Array([0])))
    const ttc = await Deno.readFile(`${fonts}/NotoSans.ttc`)
    active.addFont('NotoSans.ttc', ttc)
    // The catalog must own its copy after the host input allocation is released.
    ttc.fill(0)
    active.addFont('NotoSansSC-Regular.otf', await Deno.readFile(`${fonts}/NotoSansSC-Regular.otf`))
    const original = new TextDecoder('utf-8', { fatal: true }).decode(input)
    const missingInput = new TextEncoder().encode(original.replace(/(Dialogue:[^\n]*)/, '$1\u0378'))
    const defaultWarning = active.process(missingInput)
    require(defaultWarning.report.missing_glyph_policy === 'warn' && defaultWarning.report.warnings.length > 0, 'New engine did not default to warn')
    require(defaultWarning.report.fonts.length === 3, 'Default warning lost same-name TTC candidates')
    active.setMissingGlyphPolicy('warn')
    require(active.process(missingInput).subtitle === defaultWarning.subtitle, 'Default and explicit warn differ')
    // Historical TTC/CFF goldens intentionally remain strict.
    active.setMissingGlyphPolicy('error')
    const result = active.process(input)
    require(result.report.fonts.length === 2, 'Expected TTC and CFF subsets')
    const ttcReport = result.report.fonts.find((f: { source: string }) => f.source === 'NotoSans.ttc')
    require(ttcReport.face_index === 1, 'Wrong TTC face')
    // Golden hashes from the independently render-verified native implementation.
    const hashes = result.report.fonts.map((f: { subset_sha256: string }) => f.subset_sha256).sort()
    require(JSON.stringify(hashes) === JSON.stringify([
      '72fa9495a838e24d381e0c21874eb35b621de96b16b67fa5637826554f82072d',
      '735223fe152e3f9c81a41a45404e02258cde517b2a8bd419fa8160353e43e790',
    ]), 'Subsets differ from native baseline')
    await verifyParserCompatibility(active, original, hashes, output)
    const text: string = result.subtitle
    const [prefix, rest] = text.split('[Fonts]\n')
    const [attachments, events] = rest.split('[Events]')
    require(prefix + '[Events]' + events === original, 'Subtitle content changed')
    // A source view into WASM becomes detached when allocating a copy forces
    // growth. Rejection must free the copy with its original length and leave
    // the engine usable; it must not be mistaken for a guest trap.
    expectError(() => active.process(module.HEAPU8), 'detached')
    require(active.process(input).subtitle === text, 'Detached-input recovery changed output')
    // Force growth and re-process: an adapter retaining a stale HEAPU8 fails here.
    const previousBytes = module.HEAPU8.length
    const scratch = module._af_alloc(previousBytes)
    require(scratch !== 0 && module.HEAPU8.length > previousBytes, 'Memory did not grow')
    module._af_free(scratch, previousBytes)
    require(active.process(input).subtitle === text, 'Repeated processing changed output')
    expectError(() => active.process(missingInput), 'missing glyphs')
    active.setMissingGlyphPolicy('warn')
    const warned = active.process(missingInput)
    require(warned.report.missing_glyph_policy === 'warn' && warned.report.warnings.length > 0, 'Missing warning report')
    const probe = module._af_engine_new()
    require(module._af_set_missing_glyph_policy(probe, 2) === 0, 'Invalid policy accepted')
    module._af_engine_destroy(probe)
    active.setMissingGlyphPolicy('error')
    expectError(() => active.process(missingInput), 'missing glyphs')
    require(active.process(input).subtitle === text, 'Policy switch changed strict output')
    await Deno.mkdir(output, { recursive: true })
    await Deno.writeFile(`${output}/input.ass`, input)
    await Deno.writeTextFile(`${output}/input.assfonts.ass`, text)
    for (const [source, name] of [['NotoSans.ttc', 'ttc-only.ass'], ['NotoSansSC-Regular.otf', 'otf-only.ass']]) {
      const report = result.report.fonts.find((f: { source: string }) => f.source === source)
      const entry = attachments.split('fontname: ').slice(1).find(value => value.startsWith(report.attachment + '\n'))
      require(entry, 'Missing attachment')
      await Deno.writeTextFile(`${output}/${name}`, prefix + '[Fonts]\nfontname: ' + entry.trim() + '\n\n[Events]' + events)
    }
    await Deno.writeTextFile(`${output}/processing.json`, JSON.stringify(result.report, null, 2) + '\n')
    const legacy = new MemoryEngine(module)
    try {
      const source = makeLegacyFont(await Deno.readFile(new URL('../../vendor/harfbuzz/test/api/fonts/Roboto-Regular.abc.ttf', import.meta.url)))
      const sourceHash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', source)), b => b.toString(16).padStart(2, '0')).join('')
      legacy.addFont('legacy-prc.ttf', source)
      const template = await Deno.readTextFile(new URL('../../examples/basic.ass', import.meta.url))
      const legacyAss = template.replaceAll('Open Sans', 'Roboto').replace(/^Dialogue:.*$/gm, '').trim() +
        '\nDialogue: 0,0:00:00.00,0:00:03.00,Default,,0,0,0,,A中「」€\uE7C7\uF8F5\n'
      const input = new TextEncoder().encode(legacyAss)
      const prepared = legacy.process(input)
      require(prepared.report.missing_glyph_policy === 'warn' && prepared.report.warnings.length === 0, 'PRC glyph coverage was not recognized')
      require(prepared.report.fonts[0].source_sha256 === sourceHash && prepared.report.fonts[0].source_bytes === source.length, 'PRC normalization changed source identity')
      // Independently checked against the native CLI using this exact fixture.
      require(sourceHash === 'eafb6196d0627c7fc4e9c93348f9234253f311ee01013bbb7dae8442be38b8c5', 'PRC fixture changed')
      require(prepared.report.fonts[0].subset_sha256 === 'f0d3deded86ec3d91fe8803db821eb9de7e469bc6f948fc337db6b98ace2cbe6', 'PRC subset differs from native baseline')
      const missingInput = new TextEncoder().encode(legacyAss.replace('A中', 'A中丂\u0378\u1e3f'))
      const warned = legacy.process(missingInput)
      require(warned.report.warnings.length === 1, 'Expected PRC missing-glyph warning')
      for (const ch of ['丂', '\u0378', '\u1e3f']) require(warned.report.warnings[0].characters.includes(ch), 'PRC invented coverage or used GB18030 reassignment')
      legacy.setMissingGlyphPolicy('error')
      expectError(() => legacy.process(missingInput), 'missing glyphs')
      require(legacy.process(input).report.warnings.length === 0, 'Strict PRC coverage failed')
      await Deno.writeFile(`${output}/legacy-prc.ttf`, source)
      await Deno.writeFile(`${output}/legacy-input.ass`, input)
      await Deno.writeTextFile(`${output}/legacy-input.assfonts.ass`, prepared.subtitle)
      await Deno.writeTextFile(`${output}/legacy-processing.json`, JSON.stringify(prepared.report, null, 2) + '\n')
    } finally { legacy.close() }
    active.close()
    expectError(() => active.process(input), 'closed')
    expectError(() => active.setMissingGlyphPolicy('warn'), 'closed')
    scope.postMessage({ ok: true, checks: ['UTF-8 rejection', 'missing-font rejection', 'malformed-font recovery',
      'copied font data', 'duplicate copy avoided', 'detached-input recovery', 'response cleared', 'TTC face 1', 'CFF', 'native subset hashes', 'parser compatibility and preservation', 'ASS preservation', 'memory.grow', 'default warn', 'explicit strict goldens', 'warning policy', 'invalid policy rejection', 'strict policy recovery', 'legacy CP936 format 2', 'legacy source identity', 'legacy error/warn', 'close'],
      memory_bytes: module.HEAPU8.length, deno: Deno.version.deno })
  } catch (error) {
    scope.postMessage({ ok: false, error: String(error) })
  } finally { engine?.close() }
}
