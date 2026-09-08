// Stage measurements intentionally keep host inputs alive until input_release.
// No payloads are posted to the parent: only sizes, hashes and measurements.
import { getHeapStatistics } from 'node:v8'
import { instantiateDenoModule } from './abi.ts'

type Config = {
  build: string
  catalogs: { fonts: { path: string; label: string }[]; inputs: string[] }[]
  loops: number
  duplicates: number
  duplicateFont?: number
  cycles: number
  gc: boolean
  clearResults?: boolean
  allocationTrace?: boolean
  eachInputOnce?: boolean
  goldens?: Record<string, string>
  reuseFontInput?: boolean
  retainResults?: 'paths' | 'subtitle'
  missingGlyphPolicy?: 'error' | 'warn'
}
const scope = globalThis as unknown as {
  onmessage: (event: MessageEvent<Config>) => void
  postMessage(value: unknown): void
}
scope.onmessage = async ({ data: config }) => {
  let m: any, engine = 0, sequence = 0
  let staging = 0, stagingSize = 0
  let poisoned = false
  const guest = <T>(call: () => T): T => {
    try { return call() } catch (error) { poisoned = true; throw error }
  }
  const allocations = new Map<number, number>()
  const retained: unknown[] = []
  let requested = 0, requestedPeak = 0, phasePeak = 0, unknownFrees = 0, duplicateAllocations = 0
  const onFree = (ptr: number) => {
    if (!ptr) return
    const previous = allocations.get(ptr)
    if (previous === undefined) { unknownFrees++; return }
    requested -= previous; allocations.delete(ptr)
  }
  const onMalloc = (ptr: number, bytes: number) => {
    if (!ptr) return
    if (allocations.has(ptr)) { duplicateAllocations++; onFree(ptr) }
    allocations.set(ptr, bytes); requested += bytes
    requestedPeak = Math.max(requestedPeak, requested); phasePeak = Math.max(phasePeak, requested)
  }
  const encoder = new TextEncoder(), decoder = new TextDecoder('utf-8', { fatal: true })
  const sample = (stage: string, detail: object = {}) => {
    const heap = getHeapStatistics()
    scope.postMessage({ type: 'sample', sequence: sequence++, stage,
      timestamp_ms: Date.now(), monotonic_ms: performance.now(),
      linear_bytes: m?.HEAPU8.byteLength ?? 0,
      live_bytes: m?._af_memory_live ? guest(() => m._af_memory_live()) : null,
      free_bytes: m?._af_memory_free ? guest(() => m._af_memory_free()) : null,
      arena_bytes: m?._af_memory_arena ? guest(() => m._af_memory_arena()) : null,
      result_bytes: engine ? guest(() => m._af_result_len(engine)) : 0,
      ...Deno.memoryUsage(), v8_used_heap: heap.used_heap_size,
      v8_external: heap.external_memory,
      ...(config.allocationTrace ? { trace_requested_bytes: requested,
        trace_peak_requested_bytes: requestedPeak, trace_phase_peak_requested_bytes: phasePeak,
        trace_blocks: allocations.size, trace_unknown_frees: unknownFrees,
        trace_duplicate_allocations: duplicateAllocations } : {}), ...detail })
    phasePeak = requested
  }
  const allocate = (bytes: Uint8Array) => {
    const ptr = guest(() => m._af_alloc(bytes.length))
    if (!ptr) throw Error('Input allocation failed')
    m.HEAPU8.set(bytes, ptr)
    return ptr
  }
  const reply = (ok: number) => {
    const p = guest(() => m._af_result_ptr(engine)), n = guest(() => m._af_result_len(engine))
    const value = JSON.parse(decoder.decode(m.HEAPU8.subarray(p, p + n)))
    if (!ok) throw Error(value.error)
    return value
  }
  const add = async (font: { path: string; label: string }, detail: object) => {
    let bytes: Uint8Array | null = await Deno.readFile(font.path)
    const label = encoder.encode(font.label), n = bytes.length
    if (staging && n > stagingSize) throw Error('Font size changed after staging allocation')
    sample('font_host_read', { ...detail, input_bytes: n })
    const p = allocate(label), q = staging || allocate(bytes)
    if (staging) m.HEAPU8.set(bytes, q)
    try {
      sample('font_input_allocated', detail)
      const start = performance.now()
      const ok = guest(() => m._af_add_font(engine, p, label.length, q, n))
      sample('font_added', { ...detail, elapsed_ms: performance.now() - start, ...reply(ok) })
    } finally {
      bytes = null
      if (!poisoned) {
        if (!staging) guest(() => m._af_free(q, n))
        guest(() => m._af_free(p, label.length))
        sample('font_input_released', detail)
      }
    }
  }
  const process = async (path: string | null, detail: object) => {
    let input: Uint8Array | null = path ? await Deno.readFile(path) : new Uint8Array([255])
    const n = input.length, p = allocate(input)
    sample('process_input_allocated', { ...detail, input_bytes: n })
    try {
      const start = performance.now(), ok = guest(() => m._af_process(engine, p, n))
      sample('process_returned', { ...detail, ok, elapsed_ms: performance.now() - start })
      let result = reply(ok)
      sample('result_decoded', { ...detail, output_sha256: result.report.output_sha256,
        subset_hashes: result.report.fonts.map((f: any) => f.subset_sha256),
        subset_bytes: result.report.fonts.reduce((n: number, f: any) => n + f.subset_bytes, 0),
        characters: result.report.fonts.reduce((n: number, f: any) => n + [...f.characters].length, 0) })
      if (config.goldens && path) {
        const expected = await Deno.readFile(config.goldens[path])
        const actual = encoder.encode(result.subtitle)
        if (actual.length !== expected.length || !actual.every((byte, index) => byte === expected[index])) {
          throw Error(`Previously verified subtitle bytes changed: ${path}`)
        }
        const hash = [...new Uint8Array(await crypto.subtle.digest('SHA-256', actual))]
          .map(byte => byte.toString(16).padStart(2, '0')).join('')
        if (hash !== result.report.output_sha256) throw Error('Reported output hash differs from host hash')
        sample('golden_verified', { ...detail, input: path, output_sha256: hash,
          output_bytes: actual.length, attachments: result.report.fonts.length })
      }
      if (config.retainResults === 'subtitle') retained.push(result)
      if (config.retainResults === 'paths') retained.push({ success: true, inputFile: path, outputFile: `${path}.assfonts.ass` })
      result = null
      if (config.clearResults) {
        guest(() => m._af_result_clear(engine))
        sample('wasm_result_released', detail)
      }
    } catch (error) {
      if (!poisoned) sample('process_error', { ...detail, error: String(error) })
      if (path || poisoned) throw error
    } finally {
      input = null
      if (!poisoned) {
        guest(() => m._af_free(p, n))
        sample('input_and_js_result_released', detail)
      }
    }
  }
  try {
    sample('worker_cold')
    const { default: create } = await import(new URL(config.build + '/engine.js', 'file:///').href)
    m = await instantiateDenoModule(create, { wasmBinary: await Deno.readFile(config.build + '/assfonts-wasm.wasm'),
      ...(config.allocationTrace ? { onMalloc, onFree,
        onRealloc: (oldPtr: number, newPtr: number, bytes: number) => {
          if (newPtr) { onFree(oldPtr); onMalloc(newPtr, bytes) }
        } } : {}) })
    sample('module_loaded')
    for (let cycle = 0; cycle < config.cycles; cycle++) {
      for (const [catalog, value] of config.catalogs.entries()) {
        engine = guest(() => m._af_engine_new()); sample('engine_created', { cycle, catalog })
        if (config.missingGlyphPolicy) {
          if (m._af_set_missing_glyph_policy) {
            reply(guest(() => m._af_set_missing_glyph_policy(engine, config.missingGlyphPolicy === 'warn' ? 1 : 0)))
          } else if (config.missingGlyphPolicy !== 'error') {
            throw Error('Historical strict-only build does not support warning policy')
          }
          // The telemetry-only historical baseline predates the setter and
          // always uses error. New builds select it explicitly for parity.
        }
        if (config.reuseFontInput) {
          stagingSize = 0
          for (const font of value.fonts) stagingSize = Math.max(stagingSize, (await Deno.stat(font.path)).size)
          staging = guest(() => m._af_alloc(stagingSize))
          if (!staging) throw Error('Font staging allocation failed')
          sample('font_staging_allocated', { cycle, catalog, staging_bytes: stagingSize })
        }
        for (const [font, valueFont] of value.fonts.entries()) await add(valueFont, { cycle, catalog, font })
        sample('catalog_loaded', { cycle, catalog })
        for (let duplicate = 0; duplicate < config.duplicates; duplicate++) {
          await add(value.fonts[config.duplicateFont ?? 0], { cycle, catalog, duplicate })
        }
        if (staging) {
          guest(() => m._af_free(staging, stagingSize)); staging = 0
          sample('font_staging_released', { cycle, catalog })
        }
        await process(null, { cycle, catalog, expected_error: true })
        for (let iteration = 0; iteration < (config.eachInputOnce ? value.inputs.length : config.loops); iteration++) {
          await process(value.inputs[iteration % value.inputs.length], { cycle, catalog, iteration })
          if (config.gc && iteration % 10 === 9) {
            const gc = (globalThis as any).gc
            if (!gc) throw Error('GC requested but unavailable; pass --v8-flags=--expose-gc')
            gc(); sample('diagnostic_gc', { cycle, catalog, iteration })
          }
          // Allow ordinary runtime GC and the parent's sampler to run.
          await new Promise(resolve => setTimeout(resolve, 0))
        }
        guest(() => m._af_engine_destroy(engine)); engine = 0; sample('engine_destroyed', { cycle, catalog })
      }
    }
    if (config.retainResults) {
      sample('retained_results_before_release', { retained_count: retained.length })
      retained.length = 0
      sample('retained_results_released')
      if (config.gc) {
        ;(globalThis as any).gc()
        sample('retained_results_gc')
      }
    }
    sample('worker_idle')
    scope.postMessage({ type: 'done' })
  } catch (error) { scope.postMessage({ type: 'fatal', error: String(error) }) }
  finally {
    if (!poisoned) {
      if (staging) guest(() => m._af_free(staging, stagingSize))
      if (engine) guest(() => m._af_engine_destroy(engine))
    }
  }
}
