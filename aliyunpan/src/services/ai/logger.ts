const DEBUG = true

type LogFn = (msg: string, data?: unknown) => void

const noop = () => {}

function makeLogger(prefix: string, color: string): Record<string, LogFn> {
  if (!DEBUG) return new Proxy({}, { get: () => noop }) as any
  const style = `color:${color};font-weight:bold`
  const log = (level: string, msg: string, data?: unknown) => {
    if (data !== undefined) console.log(`%c[AI:${prefix}:${level}]%c ${msg}`, style, '', data)
    else console.log(`%c[AI:${prefix}:${level}]%c ${msg}`, style, '')
  }
  return {
    start: (msg, d) => log('start', msg, d),
    complete: (msg, d) => log('done', msg, d),
    error: (msg, d) => log('err', msg, d),
    progress: (msg, d) => log('prog', msg, d),
  }
}

export const aiLogger = {
  chunker: makeLogger('chunker', '#e67e22'),
  embedding: makeLogger('embed', '#3498db'),
  store: makeLogger('store', '#2ecc71'),
  search: makeLogger('search', '#9b59b6'),
  chat: makeLogger('chat', '#1abc9c'),
  rag: makeLogger('rag', '#e74c3c'),
  provider: makeLogger('prov', '#f39c12'),
}
