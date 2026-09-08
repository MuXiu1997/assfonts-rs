// Deliberate OOM runs only against a separately built 32 MiB module.
import { instantiateDenoModule, MemoryEngine } from './abi.ts'
const scope = globalThis as unknown as {
  onmessage: (event: MessageEvent<{ build: string; mode: string }>) => void
  postMessage(value: unknown): void
}
scope.onmessage = async ({ data: { build, mode } }) => {
  let engine = 0, m: any
  const stderr: string[] = []
  try {
    const { default: create } = await import(new URL(build + '/engine.js', 'file:///').href)
    m = await instantiateDenoModule(create, { wasmBinary: await Deno.readFile(build + '/assfonts-wasm.wasm'),
      printErr: (message: string) => stderr.push(message) })
    const font = await Deno.readFile(new URL('../../vendor/harfbuzz/test/api/fonts/OpenSans-Regular.ttf', import.meta.url))
    const subtitle = await Deno.readFile(new URL('../../examples/basic.ass', import.meta.url))
    const label = new TextEncoder().encode('test.ttf')
    const bytes = <T>(value: Uint8Array, call: (p: number, n: number) => T): T => {
      const p = m._af_alloc(value.length)
      if (!p) throw Error('Unexpected input allocation failure')
      m.HEAPU8.set(value, p)
      try { return call(p, value.length) } finally { m._af_free(p, value.length) }
    }
    if (mode === 'owned-oom') {
      const large = new Uint8Array(20 * 1024 ** 2)
      large.set(font)
      const client = new MemoryEngine(m)
      let cleanupCalls = 0
      const free = m._af_free, destroy = m._af_engine_destroy
      m._af_free = (...args: number[]) => { cleanupCalls++; return free(...args) }
      m._af_engine_destroy = (...args: number[]) => { cleanupCalls++; return destroy(...args) }
      try {
        client.addFont('test.ttf', large)
      } catch (error) {
        if (!stderr.some(line => /memory allocation.*failed/.test(line))) throw error
        let policyRejected = false
        try { client.setMissingGlyphPolicy('error') } catch (failure) {
          policyRejected = String(failure).includes('unusable after a trap')
        }
        if (!policyRejected) throw Error('Poisoned module allowed a policy change')
        client.close()
        if (cleanupCalls) throw Error('Adapter called guest cleanup after a trap')
        let rejected = false
        try { new MemoryEngine(m) } catch { rejected = true }
        if (!rejected) throw Error('Poisoned module allowed a new engine')
        scope.postMessage({ ok: true, mode, trapped: true, error: String(error), stderr,
          cleanup_calls_after_trap: cleanupCalls, new_engine_rejected: rejected,
          policy_change_rejected: policyRejected,
          linear_bytes: m.HEAPU8.length })
        return
      }
      throw Error('Expected owned allocation to exceed 32 MiB')
    }
    engine = m._af_engine_new()
    if (mode === 'refuse-input') {
      if (m._af_alloc(3 * 1024 ** 3) !== 0) throw Error('Invalid Rust layout was accepted')
      if (m._af_alloc(64 * 1024 ** 2) !== 0) throw Error('Configured maximum was exceeded')
    }
    const added = bytes(label, (p, n) => bytes(font, (q, size) => m._af_add_font(engine, p, n, q, size)))
    if (added !== 1) throw Error('Font loading failed after refused allocation')
    const ok = bytes(subtitle, (p, n) => m._af_process(engine, p, n))
    if (ok !== 1) throw Error('Processing failed after refused allocation')
    const p = m._af_result_ptr(engine), n = m._af_result_len(engine)
    const result = JSON.parse(new TextDecoder().decode(m.HEAPU8.subarray(p, p + n)))
    m._af_result_clear(engine)
    if (m._af_result_len(engine) !== 0) throw Error('Result not cleared')
    scope.postMessage({ ok: true, mode, output_sha256: result.report.output_sha256,
      linear_bytes: m.HEAPU8.length, stderr })
  } catch (error) { scope.postMessage({ ok: false, mode, error: String(error), stderr }) }
  finally { if (engine) m._af_engine_destroy(engine) }
}
