import { AssfontsError } from './errors.js'
import type { Engine, MissingGlyphPolicy, ParseMode, ProcessResult } from './types.js'

export interface WasmModule {
  HEAPU8: Uint8Array
  _af_alloc(length: number): number
  _af_free(pointer: number, length: number): void
  _af_engine_new(): number
  _af_engine_destroy(engine: number): void
  _af_add_font(engine: number, label: number, labelLength: number, data: number, length: number): number
  _af_process(engine: number, data: number, length: number): number
  _af_set_parse_mode(engine: number, mode: number): number
  _af_set_missing_glyph_policy(engine: number, policy: number): number
  _af_result_ptr(engine: number): number
  _af_result_len(engine: number): number
  _af_result_clear(engine: number): void
}

// A trap poisons every engine sharing a module. Never call guest cleanup after it.
const poisoned = new WeakSet<WasmModule>()

export class MemoryEngine implements Engine {
  #handle: number
  constructor(private readonly module: WasmModule) {
    this.#handle = this.guest(() => module._af_engine_new())
    if (!this.#handle) throw new AssfontsError('FATAL', 'Engine allocation failed')
  }

  private guest<T>(call: () => T): T {
    if (poisoned.has(this.module)) {
      throw new AssfontsError('FATAL', 'WASM instance unusable; terminate its Worker')
    }
    try { return call() } catch (cause) {
      poisoned.add(this.module)
      throw new AssfontsError('FATAL', 'WASM execution failed; terminate its Worker', { cause })
    }
  }

  private requireOpen(): void {
    if (poisoned.has(this.module)) throw new AssfontsError('FATAL', 'WASM instance unusable; terminate its Worker')
    if (!this.#handle) throw new AssfontsError('CLOSED', 'Engine closed')
  }

  private bytes<T>(value: Uint8Array, call: (pointer: number, length: number) => T): T {
    if (!(value instanceof Uint8Array)) throw new AssfontsError('INPUT', 'Expected Uint8Array')
    const length = value.byteLength
    const pointer = this.guest(() => this.module._af_alloc(length))
    if (!pointer) throw new AssfontsError('INPUT', 'Input allocation failed')
    try {
      // Reacquire HEAPU8 after allocation: memory.grow detaches old views.
      try { this.module.HEAPU8.set(value, pointer) } catch (cause) {
        throw new AssfontsError('INPUT', 'Cannot copy input bytes (buffer may be detached)', { cause })
      }
      return call(pointer, length)
    } finally {
      if (!poisoned.has(this.module)) this.guest(() => this.module._af_free(pointer, length))
    }
  }

  private response<T>(ok: number): T {
    const pointer = this.guest(() => this.module._af_result_ptr(this.#handle))
    const length = this.guest(() => this.module._af_result_len(this.#handle))
    let value: unknown
    try {
      value = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(
        this.module.HEAPU8.subarray(pointer, pointer + length),
      ))
    } catch (cause) {
      poisoned.add(this.module)
      throw new AssfontsError('FATAL', 'Invalid WASM response', { cause })
    } finally {
      if (!poisoned.has(this.module)) this.guest(() => this.module._af_result_clear(this.#handle))
    }
    if (!ok) throw new AssfontsError('INPUT', String((value as { error: string }).error))
    return value as T
  }

  addFont(label: string, bytes: Uint8Array): { faces: number } {
    this.requireOpen()
    if (typeof label !== 'string') throw new AssfontsError('INPUT', 'Expected a font label string')
    return this.bytes(new TextEncoder().encode(label), (pointer, length) =>
      this.bytes(bytes, (data, size) => this.response(this.guest(() =>
        this.module._af_add_font(this.#handle, pointer, length, data, size)))))
  }

  process(bytes: Uint8Array): ProcessResult {
    this.requireOpen()
    return this.bytes(bytes, (pointer, length) => this.response(this.guest(() =>
      this.module._af_process(this.#handle, pointer, length))))
  }

  setParseMode(mode: ParseMode): void {
    this.requireOpen()
    if (mode !== 'strict' && mode !== 'compatible') throw new AssfontsError('INPUT', 'Invalid parse mode')
    this.response(this.guest(() => this.module._af_set_parse_mode(this.#handle, mode === 'strict' ? 0 : 1)))
  }

  setMissingGlyphPolicy(policy: MissingGlyphPolicy): void {
    this.requireOpen()
    if (policy !== 'error' && policy !== 'warn') throw new AssfontsError('INPUT', 'Invalid missing-glyph policy')
    this.response(this.guest(() => this.module._af_set_missing_glyph_policy(this.#handle, policy === 'warn' ? 1 : 0)))
  }

  close(): void {
    const handle = this.#handle
    this.#handle = 0
    if (handle && !poisoned.has(this.module)) this.guest(() => this.module._af_engine_destroy(handle))
  }
}
