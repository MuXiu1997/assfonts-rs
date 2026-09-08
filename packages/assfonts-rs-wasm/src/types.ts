export type ParseMode = 'strict' | 'compatible'
export type MissingGlyphPolicy = 'error' | 'warn'

export interface FontRequest {
  family: string
  weight: number
  italic: boolean
}

export interface FontWarning {
  code: string
  request: FontRequest
  source: string
  face_index: number
  characters: string
  missing_from_all_candidates: string
}

export interface FontReport {
  requests: FontRequest[]
  source: string
  source_sha256: string
  face_index: number
  characters: string
  source_bytes: number
  subset_bytes: number
  subset_sha256: string
  attachment: string
}

export interface Report {
  schema_version: number
  backend: string
  input_sha256: string
  output_sha256: string
  fonts: FontReport[]
  missing_glyph_policy: MissingGlyphPolicy
  parse_mode: ParseMode
  warnings: FontWarning[]
}

export interface ProcessResult {
  subtitle: string
  report: Report
}

export interface EngineOptions {
  parseMode?: ParseMode
  missingGlyphPolicy?: MissingGlyphPolicy
  /** Overrides the bundled binary. Must match this package's ABI. Copied before initialization. */
  wasmBinary?: Uint8Array
}

export interface Engine {
  /** Copies and indexes the font; identical bytes retain the first label and order. */
  addFont(label: string, bytes: Uint8Array): { faces: number }
  /** Synchronous. Input must be UTF-8 ASS v4+. */
  process(bytes: Uint8Array): ProcessResult
  setParseMode(mode: ParseMode): void
  setMissingGlyphPolicy(policy: MissingGlyphPolicy): void
  /** Idempotent. Does not guarantee that the host returns RSS to the OS. */
  close(): void
}
