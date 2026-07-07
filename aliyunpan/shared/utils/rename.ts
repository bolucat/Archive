const RULE_REGEX = /\((\d+)([+-])\)/

export interface ParsedRule {
  init: number
  step: 1 | -1
  len: number
}

export const getRuleString = (out: string): string | null => {
  const m = out.match(/\((.+?)\)/)
  return m ? m[1] : null
}

export const buildRule = (rule: string): ParsedRule | null => {
  const m = rule.match(/^(\d+)([+-])$/)
  if (!m) return null
  const init = Number(m[1])
  const step: 1 | -1 = m[2] === '+' ? 1 : -1
  return { init, step, len: m[1].length }
}

export const buildOuts = (uris: string[], out: string): string[] => {
  if (!out) return []
  const m = out.match(RULE_REGEX)
  if (!m) return uris.map(() => out)
  const rule = buildRule(m[0].slice(1, -1))
  if (!rule) return []
  return uris.map((_, i) => {
    const n = rule.init + rule.step * i
    const padded = String(n).padStart(rule.len, '0')
    return out.replace(RULE_REGEX, padded)
  })
}
