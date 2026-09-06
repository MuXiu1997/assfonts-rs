// Test/reference host adapter. No dependency on the subtitle workflow library.
type Module = {
  HEAPU8: Uint8Array
  _af_alloc(length: number): number
  _af_free(pointer: number, length: number): void
  _af_engine_new(): number
  _af_engine_destroy(engine: number): void
  _af_add_font(engine: number, label: number, labelLength: number, bytes: number, length: number): number
  _af_process(engine: number, bytes: number, length: number): number
  _af_result_ptr(engine: number): number
  _af_result_len(engine: number): number
}

export class MemoryEngine {
  private engine: number
  constructor(readonly module: Module) {
    this.engine = module._af_engine_new()
    if (!this.engine) throw new Error('Engine allocation failed')
  }

  private bytes<T>(value: Uint8Array, call: (pointer: number, length: number) => T): T {
    const pointer = this.module._af_alloc(value.length)
    if (!pointer) throw new Error('Input allocation failed')
    try {
      // Do not cache this view: af_alloc/add_font/process may grow memory.
      this.module.HEAPU8.set(value, pointer)
      return call(pointer, value.length)
    } finally { this.module._af_free(pointer, value.length) }
  }

  private response(ok: number) {
    const pointer = this.module._af_result_ptr(this.engine)
    const length = this.module._af_result_len(this.engine)
    const result = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(
      this.module.HEAPU8.subarray(pointer, pointer + length),
    ))
    if (!ok) throw new Error(result.error)
    return result
  }

  addFont(label: string, bytes: Uint8Array) {
    if (!this.engine) throw new Error('Engine closed')
    return this.bytes(new TextEncoder().encode(label), (pointer, length) =>
      this.bytes(bytes, (data, size) =>
        this.response(this.module._af_add_font(this.engine, pointer, length, data, size))))
  }

  process(bytes: Uint8Array) {
    if (!this.engine) throw new Error('Engine closed')
    return this.bytes(bytes, (pointer, length) =>
      this.response(this.module._af_process(this.engine, pointer, length)))
  }

  close() {
    if (this.engine) this.module._af_engine_destroy(this.engine)
    this.engine = 0
  }
}

export async function loadModule() {
  const binary = await Deno.readFile(new URL('../../target/wasm/assfonts-wasm.wasm', import.meta.url))
  const { default: createModule } = await import(new URL('../../target/wasm/engine.js', import.meta.url).href)
  return await createModule({ wasmBinary: binary }) as Module
}
