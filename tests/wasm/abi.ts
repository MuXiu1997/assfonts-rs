// Test/reference host adapter. No dependency on the subtitle workflow library.
type Module = {
  HEAPU8: Uint8Array
  _af_alloc(length: number): number
  _af_free(pointer: number, length: number): void
  _af_engine_new(): number
  _af_engine_destroy(engine: number): void
  _af_add_font(engine: number, label: number, labelLength: number, bytes: number, length: number): number
  _af_process(engine: number, bytes: number, length: number): number
  _af_set_missing_glyph_policy(engine: number, policy: number): number
  _af_set_parse_mode(engine: number, mode: number): number
  _af_result_ptr(engine: number): number
  _af_result_len(engine: number): number
  _af_result_clear(engine: number): void
}

// A trap can leave Rust/C++ frames and ownership transfers incomplete. Poison
// the entire module, including other engines, and let the host terminate its
// Worker. In particular, do not run allocation cleanup after an abort.
const poisoned = new WeakSet<Module>()

// Deno exposes a Node-compatible global `process` even in a Web Worker.
// Emscripten's assertion build mistakes that shim for a Node host and rejects
// ENVIRONMENT=web,worker. Select the actual Worker path during initialization;
// restore the exact property afterwards. No guest memory checks are disabled.
export async function instantiateDenoModule(createModule: (options: any) => Promise<any>, options: any) {
  if (!('WorkerGlobalScope' in globalThis)) throw new Error('WASM test module must run in a Worker')
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'process')
  if (descriptor && !descriptor.configurable) throw new Error('Cannot select Deno Worker environment')
  if (descriptor) Object.defineProperty(globalThis, 'process', { configurable: true, value: undefined })
  try { return await createModule(options) }
  finally { if (descriptor) Object.defineProperty(globalThis, 'process', descriptor) }
}

export class MemoryEngine {
  private engine: number
  constructor(readonly module: Module) {
    // Inherit strict syntax and warn-on-missing-glyphs defaults independently.
    if (poisoned.has(module)) throw new Error('WASM instance unusable after a trap; terminate its Worker')
    this.engine = this.guest(() => module._af_engine_new())
    if (!this.engine) throw new Error('Engine allocation failed')
  }

  private guest<T>(call: () => T): T {
    try { return call() } catch (error) {
      poisoned.add(this.module)
      throw error
    }
  }

  private requireOpen() {
    if (poisoned.has(this.module)) throw new Error('WASM instance unusable after a trap; terminate its Worker')
    if (!this.engine) throw new Error('Engine closed')
  }

  private bytes<T>(value: Uint8Array, call: (pointer: number, length: number) => T): T {
    // A caller may pass a view that becomes detached by memory.grow. Always
    // release with the original allocation length, even if copying then fails.
    const length = value.length
    const pointer = this.guest(() => this.module._af_alloc(length))
    if (!pointer) throw new Error('Input allocation failed')
    try {
      // Do not cache this view: af_alloc/add_font/process may grow memory.
      this.module.HEAPU8.set(value, pointer)
      return call(pointer, length)
    } finally {
      if (!poisoned.has(this.module)) this.guest(() => this.module._af_free(pointer, length))
    }
  }

  private response(ok: number) {
    const pointer = this.guest(() => this.module._af_result_ptr(this.engine))
    const length = this.guest(() => this.module._af_result_len(this.engine))
    let result
    try {
      result = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(
        this.module.HEAPU8.subarray(pointer, pointer + length),
      ))
    } finally { this.guest(() => this.module._af_result_clear(this.engine)) }
    if (!ok) throw new Error(result.error)
    return result
  }

  addFont(label: string, bytes: Uint8Array) {
    this.requireOpen()
    return this.bytes(new TextEncoder().encode(label), (pointer, length) =>
      this.bytes(bytes, (data, size) =>
        this.response(this.guest(() => this.module._af_add_font(this.engine, pointer, length, data, size)))))
  }

  process(bytes: Uint8Array) {
    this.requireOpen()
    return this.bytes(bytes, (pointer, length) =>
      this.response(this.guest(() => this.module._af_process(this.engine, pointer, length))))
  }

  setMissingGlyphPolicy(policy: 'error' | 'warn') {
    this.requireOpen()
    if (policy !== 'error' && policy !== 'warn') throw new Error('Invalid missing-glyph policy')
    return this.response(this.guest(() => this.module._af_set_missing_glyph_policy(this.engine, policy === 'warn' ? 1 : 0)))
  }

  setParseMode(mode: 'strict' | 'compatible') {
    this.requireOpen()
    if (mode !== 'strict' && mode !== 'compatible') throw new Error('Invalid parse mode')
    return this.response(this.guest(() => this.module._af_set_parse_mode(this.engine, mode === 'strict' ? 0 : 1)))
  }

  close() {
    if (this.engine && !poisoned.has(this.module)) this.guest(() => this.module._af_engine_destroy(this.engine))
    this.engine = 0
  }
}

export async function loadModule() {
  const binary = await Deno.readFile(new URL('../../target/wasm/assfonts-wasm.wasm', import.meta.url))
  const { default: createModule } = await import(new URL('../../target/wasm/engine.js', import.meta.url).href)
  return await instantiateDenoModule(createModule, { wasmBinary: binary }) as Module
}
