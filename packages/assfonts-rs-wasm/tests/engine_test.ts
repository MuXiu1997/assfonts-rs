import { MemoryEngine, type WasmModule } from '../src/engine.ts'
import { AssfontsError } from '../src/errors.ts'

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

function fixture() {
  const memory = new WebAssembly.Memory({ initial: 1, maximum: 4 })
  let cleanup = 0
  const module: WasmModule = {
    get HEAPU8() { return new Uint8Array(memory.buffer) },
    _af_alloc: () => 16,
    _af_free: () => { cleanup++ },
    _af_engine_new: () => 1,
    _af_engine_destroy: () => { cleanup++ },
    _af_add_font: () => 1,
    _af_process: () => 1,
    _af_set_parse_mode: () => 1,
    _af_set_missing_glyph_policy: () => 1,
    _af_result_ptr: () => 128,
    _af_result_len: () => 2,
    _af_result_clear: () => { cleanup++ },
  }
  module.HEAPU8.set(new TextEncoder().encode('{}'), 128)
  return { module, memory, cleanup: () => cleanup }
}

Deno.test('trap poisons the module and prevents all guest cleanup and reuse', () => {
  const state = fixture()
  const first = new MemoryEngine(state.module)
  const second = new MemoryEngine(state.module)
  state.module._af_process = () => { throw new WebAssembly.RuntimeError('unreachable') }
  for (const action of [() => first.process(new Uint8Array([1])), () => second.process(new Uint8Array([1])), () => new MemoryEngine(state.module)]) {
    try { action(); throw new Error('Expected fatal error') } catch (error) {
      assert(error instanceof AssfontsError && error.code === 'FATAL', 'Wrong fatal classification')
    }
  }
  first.close()
  second.close()
  assert(state.cleanup() === 0, 'Guest cleanup ran after trap')
})

Deno.test('growth refreshes heap views and frees original input length after detachment', () => {
  const state = fixture()
  const engine = new MemoryEngine(state.module)
  const source = new Uint8Array(state.memory.buffer, 0, 5)
  let freed = -1
  state.module._af_alloc = () => { state.memory.grow(1); return 16 }
  state.module._af_free = (_, length) => { freed = length }
  try { engine.process(source); throw new Error('Expected detached input error') } catch (error) {
    assert(error instanceof AssfontsError && error.code === 'INPUT' && error.cause instanceof TypeError, 'Expected detached buffer INPUT error')
  }
  assert(freed === 5, 'Freed with detached length instead of original length')
  engine.process(new Uint8Array([7]))
  assert(state.module.HEAPU8[16] === 7, 'Input used a stale heap view')
  engine.close()
})
