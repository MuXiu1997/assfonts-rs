import { MemoryEngine, type WasmModule } from './engine.js'
import { AssfontsError } from './errors.js'
import type { Engine, EngineOptions } from './types.js'

type DenoHost = { readFile(path: URL): Promise<Uint8Array> }
// Serialize the temporary environment override across simultaneous factories.
let initialization: Promise<unknown> = Promise.resolve()

export async function createEngine(options: EngineOptions = {}): Promise<Engine> {
  const { parseMode = 'strict', missingGlyphPolicy = 'warn' } = options
  if (parseMode !== 'strict' && parseMode !== 'compatible') throw new AssfontsError('INPUT', 'Invalid parse mode')
  if (missingGlyphPolicy !== 'error' && missingGlyphPolicy !== 'warn') throw new AssfontsError('INPUT', 'Invalid missing-glyph policy')
  const deno = (globalThis as unknown as { Deno?: DenoHost }).Deno
  if (!deno || !('WorkerGlobalScope' in globalThis)) {
    throw new AssfontsError('INITIALIZATION', 'This release supports Deno Workers; initialize inside a module Worker')
  }
  let binary: Uint8Array
  try {
    binary = options.wasmBinary === undefined
      ? await deno.readFile(new URL('./wasm/assfonts-wasm.wasm', import.meta.url))
      : options.wasmBinary.slice()
  } catch (cause) {
    throw new AssfontsError('INITIALIZATION', 'Cannot read WASM binary', { cause })
  }
  const pending = initialization.then(async () => {
    const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'process')
    if (descriptor && !descriptor.configurable) throw new Error('Cannot select Deno Worker environment')
    if (descriptor) Object.defineProperty(globalThis, 'process', { configurable: true, value: undefined })
    try {
      const { default: factory } = await import(new URL('./wasm/engine.js', import.meta.url).href) as {
        default(options: { wasmBinary: Uint8Array }): Promise<WasmModule>
      }
      return await factory({ wasmBinary: binary })
    } finally {
      if (descriptor) Object.defineProperty(globalThis, 'process', descriptor)
    }
  })
  initialization = pending.then(() => undefined, () => undefined)
  let module: WasmModule
  try { module = await pending } catch (cause) {
    throw new AssfontsError('INITIALIZATION', 'Cannot initialize WASM module', { cause })
  }
  const engine = new MemoryEngine(module)
  try {
    engine.setParseMode(parseMode)
    engine.setMissingGlyphPolicy(missingGlyphPolicy)
    return engine
  } catch (error) {
    engine.close()
    throw error
  }
}
