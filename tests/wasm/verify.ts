// Run the actual artifact in a Worker, with --deny-run and without npm/jsr dependencies.
const [fonts, output] = Deno.args
if (!fonts || !output) throw new Error('Usage: verify.ts FONT_DIRECTORY NEW_OUTPUT_DIRECTORY')
const manifest = JSON.parse(await Deno.readTextFile(new URL('../../target/wasm/build-manifest.json', import.meta.url)))
async function checkArtifacts() {
  for (const name of ['engine.js', 'assfonts-wasm.wasm']) {
    const bytes = await Deno.readFile(new URL('../../target/wasm/' + name, import.meta.url))
    const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)),
      byte => byte.toString(16).padStart(2, '0')).join('')
    if (hash !== manifest.artifacts[name].sha256) throw new Error(`Artifact checksum mismatch: ${name}`)
  }
}
await checkArtifacts()
try {
  await Deno.stat(output)
  throw new Error('Output exists; choose a fresh validation directory')
} catch (error) {
  if (!(error instanceof Deno.errors.NotFound)) throw error
}
const worker = new Worker(new URL('./worker.ts', import.meta.url).href, { type: 'module' })
let timer: ReturnType<typeof setTimeout> | undefined
try {
  const result = await new Promise<{ ok: boolean; error?: string }>((resolve, reject) => {
    timer = setTimeout(() => reject(new Error('WASM verification timed out')), 60_000)
    worker.onmessage = ({ data }) => resolve(data)
    worker.onerror = (event) => { event.preventDefault(); reject(new Error(event.message)) }
    worker.postMessage({ fonts, output })
  })
  if (!result.ok) throw new Error(result.error)
  await checkArtifacts()
  await Deno.writeTextFile(`${output}/wasm-check.json`, JSON.stringify(result, null, 2) + '\n')
  console.log(JSON.stringify(result))
} finally {
  clearTimeout(timer)
  worker.terminate()
}
