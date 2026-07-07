import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { randomUUID } from 'node:crypto'
import { runBoxPlayerCli } from './commands.mjs'
import { getToolInput, listMcpTools, toolNameToCommand } from './mcpToolSchema.mjs'

export const TOOLS = listMcpTools()

function respond(id, result) {
  const msg = JSON.stringify({ jsonrpc: '2.0', id, result })
  process.stdout.write(`Content-Length: ${Buffer.byteLength(msg)}\r\n\r\n${msg}`)
}

function respondError(id, code, message) {
  const msg = JSON.stringify({ jsonrpc: '2.0', id, error: { code, message } })
  process.stdout.write(`Content-Length: ${Buffer.byteLength(msg)}\r\n\r\n${msg}`)
}

function isPlainObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value)
}

async function createTempJson(value, temp) {
  if (!temp.dir) temp.dir = await mkdtemp(join(tmpdir(), 'clouddrive-mcp-'))
  const file = join(temp.dir, `${randomUUID()}.json`)
  await writeFile(file, JSON.stringify(value), 'utf8')
  return file
}

async function normalizeInputFiles(command, input, temp) {
  const normalized = { ...(input || {}) }

  if (!getToolInput(normalized, 'input') && Array.isArray(normalized.items)) {
    const itemInputCommands = new Set(['media scan', 'media match'])
    if (itemInputCommands.has(command.command)) normalized.input = await createTempJson(normalized.items, temp)
  }

  return normalized
}

async function materializeValue(value, field, temp) {
  if (field.type === 'path' && isPlainObject(value)) return createTempJson(value, temp)
  if (field.type === 'path' && Array.isArray(value)) return createTempJson(value, temp)
  return value
}

async function buildArgv(command, input, temp) {
  const argv = command.command.split(/\s+/)

  for (const arg of command.args || []) {
    const value = await materializeValue(getToolInput(input, arg.name), arg, temp)
    if (value === undefined || value === null || value === '') {
      if (arg.required) return { error: `Missing required argument: ${arg.name}` }
      continue
    }
    argv.push(String(value))
  }

  for (const option of command.options || []) {
    if (option.name === 'json' || option.name === 'format') continue
    let value = await materializeValue(getToolInput(input, option.name), option, temp)
    if (option.name === 'dry-run' && command.requiresDryRun && value !== false) value = true
    if (value === undefined || value === null || value === '') {
      if (option.required) return { error: `Missing required option: ${option.name}` }
      continue
    }
    if (option.type === 'boolean') {
      if (value === true) argv.push(`--${option.name}`)
      continue
    }
    argv.push(`--${option.name}`, String(value))
  }

  argv.push('--json')
  return { argv }
}

async function callTool(name, input, env) {
  const command = toolNameToCommand(name)
  if (!command) return { error: `Unknown tool: ${name}` }

  const temp = { dir: '' }
  try {
    const normalizedInput = await normalizeInputFiles(command, input, temp)
    const { argv, error } = await buildArgv(command, normalizedInput, temp)
    if (error) return { error, exitCode: 1 }

    const result = await runBoxPlayerCli(argv, env)
    if (result.exitCode !== 0) {
      const errorText = result.stderr.trim() || result.stdout.trim() || 'Command failed'
      try {
        return { ...JSON.parse(errorText), exitCode: result.exitCode }
      } catch {
        return { error: errorText, exitCode: result.exitCode }
      }
    }
    try {
      return JSON.parse(result.stdout)
    } catch {
      return result.stdout.trim()
    }
  } finally {
    if (temp.dir) await rm(temp.dir, { recursive: true, force: true }).catch(() => {})
  }
}

export async function runMcpServer(env = {}) {
  const chunks = []
  let expectedLength = -1

  process.stdin.on('data', async (chunk) => {
    chunks.push(chunk)
    const buf = Buffer.concat(chunks)
    const str = buf.toString('utf8')

    const headerEnd = str.indexOf('\r\n\r\n')
    if (headerEnd === -1) return

    if (expectedLength === -1) {
      const header = str.slice(0, headerEnd)
      const match = header.match(/Content-Length:\s*(\d+)/i)
      if (!match) return
      expectedLength = parseInt(match[1], 10)
    }

    const bodyStart = Buffer.byteLength(str.slice(0, headerEnd + 4))
    const body = buf.slice(bodyStart)
    if (body.length < expectedLength) return

    const requestBody = body.slice(0, expectedLength).toString('utf8')
    chunks.length = 0
    expectedLength = -1

    let req
    try {
      req = JSON.parse(requestBody)
    } catch {
      respondError(null, -32700, 'Parse error')
      return
    }

    const { id, method, params } = req

    if (method === 'initialize') {
      respond(id, {
        protocolVersion: '2024-11-05',
        capabilities: { tools: {} },
        serverInfo: { name: 'clouddrive-cli', version: '1.0.0' },
      })
      return
    }

    if (method === 'tools/list') {
      respond(id, { tools: TOOLS })
      return
    }

    if (method === 'tools/call') {
      const toolName = params?.name
      const toolInput = params?.arguments || {}
      try {
        const result = await callTool(toolName, toolInput, env)
        respond(id, {
          content: [{ type: 'text', text: JSON.stringify(result, null, 2) }],
        })
      } catch (e) {
        respondError(id, -32603, e?.message || 'Internal error')
      }
      return
    }

    if (method === 'notifications/initialized') return

    respondError(id, -32601, `Method not found: ${method}`)
  })

  process.stdin.resume()
}
