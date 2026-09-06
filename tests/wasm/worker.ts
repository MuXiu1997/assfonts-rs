import { loadModule, MemoryEngine } from './abi.ts'

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
    const result = active.process(input)
    require(result.report.fonts.length === 2, 'Expected TTC and CFF subsets')
    const ttcReport = result.report.fonts.find((f: { source: string }) => f.source === 'NotoSans.ttc')
    require(ttcReport.face_index === 1, 'Wrong TTC face')
    // Golden hashes from the independently render-verified native implementation.
    const hashes = result.report.fonts.map((f: { subset_sha256: string }) => f.subset_sha256).sort()
    require(JSON.stringify(hashes) === JSON.stringify([
      '1eff4419296e90028c5cbe972094e6204b1af515ce2edd177f01e4857d70ecfb',
      'c10cb9370954444faaaed28f900fbb122f1cdb674159d86835c8095413332999',
    ]), 'Subsets differ from native baseline')
    const text: string = result.subtitle
    const [prefix, rest] = text.split('[Fonts]\n')
    const [attachments, events] = rest.split('[Events]')
    const original = new TextDecoder('utf-8', { fatal: true }).decode(input)
    require(prefix + '[Events]' + events === original, 'Subtitle content changed')
    // Force growth and re-process: an adapter retaining a stale HEAPU8 fails here.
    const previousBytes = module.HEAPU8.length
    const scratch = module._af_alloc(previousBytes)
    require(scratch !== 0 && module.HEAPU8.length > previousBytes, 'Memory did not grow')
    module._af_free(scratch, previousBytes)
    require(active.process(input).subtitle === text, 'Repeated processing changed output')
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
    active.close()
    expectError(() => active.process(input), 'closed')
    scope.postMessage({ ok: true, checks: ['UTF-8 rejection', 'missing-font rejection', 'malformed-font recovery',
      'copied font data', 'TTC face 1', 'CFF', 'native subset hashes', 'ASS preservation', 'memory.grow', 'close'],
      memory_bytes: module.HEAPU8.length, deno: Deno.version.deno })
  } catch (error) {
    scope.postMessage({ ok: false, error: String(error) })
  } finally { engine?.close() }
}
