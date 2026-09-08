import { AssfontsError, createEngine } from '@muxiu1997/assfonts-rs-wasm'

try {
  await createEngine()
  throw new Error('Main-thread initialization unexpectedly succeeded')
} catch (error) {
  if (!(error instanceof AssfontsError) || error.code !== 'INITIALIZATION') throw error
}
const worker = new Worker(new URL('./worker.ts', import.meta.url).href, { type: 'module' })
try {
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('Worker timeout')), 30_000)
    worker.onmessage = ({ data }) => {
      clearTimeout(timer)
      if (data !== 'ok') reject(new Error(String(data)))
      else resolve()
    }
    worker.onerror = (event) => {
      event.preventDefault()
      clearTimeout(timer)
      reject(new Error(event.message))
    }
  })
} finally { worker.terminate() }
console.log('Installed npm package: Deno Worker validation passed')
