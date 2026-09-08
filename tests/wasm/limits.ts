const [build, output] = Deno.args
if (!build || !output) throw Error('Usage: limits.ts 32_MIB_BUILD OUTPUT.json')
const manifest = JSON.parse(await Deno.readTextFile(build + '/build-manifest.json'))
if (manifest.maximum_memory_bytes !== 32 * 1024 ** 2) throw Error('Refusing to test OOM without a 32 MiB build')
async function run(mode: string) {
  const worker = new Worker(new URL('./limits-worker.ts', import.meta.url).href, { type: 'module' })
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    return await new Promise<any>((resolve, reject) => {
      timer = setTimeout(() => reject(Error('Limit test timed out')), 20_000)
      worker.onerror = event => { event.preventDefault(); reject(Error(event.message)) }
      worker.onmessage = ({ data }) => data.ok ? resolve(data) : reject(Error(JSON.stringify(data)))
      worker.postMessage({ build, mode })
    })
  } finally { clearTimeout(timer); worker.terminate() }
}
const refused = await run('refuse-input')
const trapped = await run('owned-oom')
const recovered = await run('new-worker')
if (refused.output_sha256 !== recovered.output_sha256) throw Error('New Worker output differs')
await Deno.writeTextFile(output, JSON.stringify({ manifest, refused, trapped, recovered }, null, 2) + '\n', { createNew: true })
console.log(JSON.stringify({ refused, trapped, recovered }))
