// deno run --allow-read --allow-write --deny-run tests/wasm/memory.ts CONFIG OUTPUT.jsonl
const [configPath, output] = Deno.args
if (!configPath || !output) throw Error('Usage: memory.ts CONFIG OUTPUT.jsonl')
const config = JSON.parse(await Deno.readTextFile(configPath))
if (config.missingGlyphPolicy !== undefined && !['error', 'warn'].includes(config.missingGlyphPolicy)) {
  throw Error('Invalid missingGlyphPolicy')
}
const manifest = JSON.parse(await Deno.readTextFile(config.build + '/build-manifest.json'))
async function verifyArtifacts() {
  for (const name of ['engine.js', 'assfonts-wasm.wasm']) {
    const bytes = await Deno.readFile(config.build + '/' + name)
    const hash = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
      .map(byte => byte.toString(16).padStart(2, '0')).join('')
    if (hash !== manifest.artifacts[name].sha256) throw Error(`Artifact checksum mismatch: ${name}`)
  }
}
await verifyArtifacts()
for (const [name, maximum] of [['loops', 200], ['cycles', 10], ['duplicates', 100],
  ['restarts', 10], ['concurrency', 2], ['idleAfterTerminationMs', 5000]] as const) {
  const value = config[name] ?? 1
  if (!Number.isInteger(value) || value < 0 || value > maximum) throw Error(`Unsafe ${name}: ${value}`)
}
const log = await Deno.open(output, { write: true, createNew: true })
const encoder = new TextEncoder()
const write = (value: object) => log.writeSync(encoder.encode(JSON.stringify(value) + '\n'))
const active = new Set<(reason: Error) => void>()
write({ type: 'metadata', config, deno: Deno.version, pid: Deno.pid,
  timestamp_ms: Date.now(), build: manifest, artifacts_verified_before: true })
const monitor = setInterval(() => {
  const usage = Deno.memoryUsage()
  write({ type: 'parent_sample', timestamp_ms: Date.now(), ...usage })
  if (usage.rss > 4 * 1024 ** 3) {
    for (const stop of active) stop(Error('Controlled RSS limit exceeded (4 GiB)'))
  }
}, 20)
async function run(workerId: string) {
  const worker = new Worker(new URL('./memory-worker.ts', import.meta.url).href, { type: 'module' })
  let timer: ReturnType<typeof setTimeout> | undefined
  let stop: (reason: Error) => void = () => {}
  try {
    await new Promise<void>((resolve, reject) => {
      stop = reason => { worker.terminate(); reject(reason) }
      active.add(stop)
      timer = setTimeout(() => reject(Error('Worker exceeded controlled 120s deadline')), 120_000)
      worker.onerror = event => { event.preventDefault(); reject(Error(event.message)) }
      worker.onmessage = ({ data }) => {
        write({ worker: workerId, ...data })
        if (data.type === 'fatal') reject(Error(data.error))
        if (data.type === 'done') resolve()
      }
      worker.postMessage(config)
    })
  } finally {
    active.delete(stop)
    clearTimeout(timer); worker.terminate()
    write({ type: 'worker_terminated', worker: workerId, timestamp_ms: Date.now(), ...Deno.memoryUsage() })
  }
}
try {
  for (let restart = 0; restart < (config.restarts ?? 1); restart++) {
    const runs = await Promise.allSettled(Array.from({ length: config.concurrency ?? 1 }, (_, n) => run(`${restart}:${n}`)))
    const failure = runs.find(result => result.status === 'rejected')
    if (failure?.status === 'rejected') throw failure.reason
    await new Promise(resolve => setTimeout(resolve, config.idleAfterTerminationMs ?? 250))
    write({ type: 'post_termination_idle', restart, timestamp_ms: Date.now(), ...Deno.memoryUsage() })
  }
  await verifyArtifacts()
  write({ type: 'artifacts_verified_after', timestamp_ms: Date.now() })
} finally { clearInterval(monitor); log.close() }
