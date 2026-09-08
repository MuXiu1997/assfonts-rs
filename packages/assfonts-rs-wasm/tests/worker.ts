/// <reference lib="webworker" />
import { AssfontsError, createEngine, type ProcessResult } from '@muxiu1997/assfonts-rs-wasm'

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message)
}
function inputError(fn: () => unknown) {
  try { fn() } catch (error) {
    assert(error instanceof AssfontsError && error.code === 'INPUT', 'Expected INPUT error')
    return
  }
  throw new Error('Expected rejection')
}

const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'process')
const input = await Deno.readFile(new URL('./input.ass', import.meta.url))
const font = await Deno.readFile(new URL('./font.ttf', import.meta.url))
const [engine, second] = await Promise.all([
  createEngine(),
  createEngine({ wasmBinary: await Deno.readFile(new URL('./binary.wasm', import.meta.url)) }),
])
try {
  assert(Object.getOwnPropertyDescriptor(globalThis, 'process')?.value === descriptor?.value, 'process global not restored')
  inputError(() => engine.process(input))
  inputError(() => engine.addFont('broken', new Uint8Array([1, 2, 3])))
  const supplied = font.slice()
  assert(engine.addFont('font.ttf', supplied).faces === 1, 'Expected one face')
  supplied.fill(0)
  assert(engine.addFont('duplicate.ttf', font).faces === 1, 'Font was not deduplicated')
  const result: ProcessResult = engine.process(input)
  assert(result.report.parse_mode === 'strict' && result.report.missing_glyph_policy === 'warn', 'Wrong defaults')
  assert(result.report.fonts.length === 1 && result.report.fonts[0].source === 'font.ttf', 'Wrong font report')
  const original = new TextDecoder().decode(input)
  const embedded = result.subtitle.indexOf('[Fonts]')
  const events = result.subtitle.indexOf('[Events]', embedded)
  assert(embedded >= 0 && events > embedded, 'Missing attachment')
  assert(result.subtitle.slice(0, embedded) + result.subtitle.slice(events) === original, 'Original subtitle changed')
  inputError(() => engine.process(new Uint8Array([255])))
  assert(engine.process(input).subtitle === result.subtitle, 'Input error prevented recovery')
  engine.setParseMode('compatible')
  engine.setMissingGlyphPolicy('error')
  assert(engine.process(input).report.parse_mode === 'compatible', 'Mode setter failed')
  second.addFont('font.ttf', font)
  assert(second.process(input).subtitle === result.subtitle, 'Custom binary changed result')
  engine.close()
  engine.close()
  try {
    engine.process(input)
    throw new Error('Closed engine accepted input')
  } catch (error) {
    assert(error instanceof AssfontsError && error.code === 'CLOSED', 'Expected CLOSED error')
  }
} finally {
  engine.close()
  second.close()
}
postMessage('ok')
