export type AssfontsErrorCode = 'INPUT' | 'CLOSED' | 'FATAL' | 'INITIALIZATION'

export class AssfontsError extends Error {
  constructor(readonly code: AssfontsErrorCode, message: string, options?: ErrorOptions) {
    super(message, options)
    this.name = 'AssfontsError'
  }
}
