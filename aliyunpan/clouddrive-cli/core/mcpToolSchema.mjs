import { listCommands } from './commandManifest.mjs'

const JSON_OPTIONS = new Set(['json', 'format'])

const DEFAULTS = {
  account: 'default',
  provider: 'aliyun',
  limit: 100,
}

const DESCRIPTIONS = {
  account: 'Account id or "default".',
  apply: 'Set true to execute a destructive trash/delete plan. Defaults to preview only.',
  browser: 'Browser command/name to open for OAuth login.',
  'confirm-apply': 'Required by MCP when dry-run is explicitly false for commands that normally require dry-run first.',
  current: 'Optional current FileItem JSON path used to validate a rename plan.',
  default: 'Set the imported account as the default account for the provider.',
  depth: 'Maximum recursion depth.',
  'drive-id': 'Drive id. Uses provider/account default when omitted.',
  'dry-run': 'Preview without making changes. Defaults to true for commands that require dry-run first.',
  'file-id': 'File or folder id.',
  cursor: 'Pagination cursor returned by the previous page.',
  local: 'Local file or directory path.',
  name: 'Name or display name, depending on command.',
  output: 'Output path. For downloads this is the local file path; for large read commands this is a JSON output path.',
  parent: 'Parent folder id.',
  plan: 'Path to a plan JSON file. For MCP calls, a JSON plan object is also accepted and written to a temporary file.',
  provider: 'Provider id such as aliyun, cloud123, 115, 139, 189, quark, baidu, pikpak, onedrive, box, or dropbox.',
  query: 'Provider-specific raw search query.',
  rationale: 'Short reason for this write operation. Stored in results/logs for auditability.',
  'redirect-uri': 'OAuth redirect URI. Use a registered loopback URI when the provider requires one.',
  'remote-parent': 'Remote parent folder id for upload planning.',
  token: 'Path to a token JSON file.',
}

const ENUMS = {
  provider: ['aliyun', 'cloud123', '115', '139', '189', 'quark', 'baidu', 'pikpak', 'onedrive', 'box', 'dropbox'],
}

const ALIASES = {
  drive_id: ['driveId'],
  file_id: ['fileId'],
  operation_id: ['id', 'operationId'],
  redirect_uri: ['redirectUri'],
  remote_parent: ['remoteParent'],
}

export function commandToToolName(command) {
  return command.replace(/\s+/g, '_').replace(/-/g, '_')
}

export function toolNameToCommand(toolName) {
  return listCommands().find((command) => commandToToolName(command.command) === toolName) || null
}

function inputName(name) {
  return name.replace(/-/g, '_')
}

function schemaType(field) {
  if (field.type === 'number') return 'number'
  if (field.type === 'boolean') return 'boolean'
  return 'string'
}

function schemaForField(field, command) {
  const key = inputName(field.name)
  const schema = {
    type: schemaType(field),
    description: DESCRIPTIONS[field.name] || DESCRIPTIONS[key] || `${field.name} parameter.`,
  }
  if (field.type === 'path') schema.description = `${schema.description} Use an absolute path when possible.`
  if (DEFAULTS[field.name] !== undefined) schema.default = DEFAULTS[field.name]
  if (field.name === 'dry-run' && command.requiresDryRun) schema.default = true
  if (ENUMS[field.name]) schema.enum = ENUMS[field.name]
  return schema
}

function describeCommand(command) {
  const tags = []
  if (command.access === 'write') tags.push('write')
  if (command.requiresDryRun) tags.push('dry-run first')
  if (command.destructive) tags.push('destructive')
  if (command.undoable) tags.push('undoable')
  const suffix = tags.length ? ` [${tags.join(', ')}]` : ''
  return `${command.description} CLI: clouddrive-cli ${command.command}.${suffix}`
}

export function commandToMcpTool(command) {
  const properties = {}
  const required = []
  for (const field of [...(command.args || []), ...(command.options || [])]) {
    if (JSON_OPTIONS.has(field.name)) continue
    const key = inputName(field.name)
    properties[key] = schemaForField(field, command)
    if (field.required) required.push(key)
  }

  return {
    name: commandToToolName(command.command),
    description: describeCommand(command),
    inputSchema: {
      type: 'object',
      properties,
      required,
    },
    annotations: {
      readOnlyHint: command.access !== 'write',
      destructiveHint: !!command.destructive,
      idempotentHint: false,
      openWorldHint: true,
    },
    _meta: {
      command: command.command,
      group: command.group,
      access: command.access,
      output: command.output,
      requiresDryRun: !!command.requiresDryRun,
      undoable: !!command.undoable,
      largeOutput: !!command.largeOutput,
      examples: command.examples || [],
      safety: command.safety || {},
      providerRequirements: command.providerRequirements || null,
    },
  }
}

export function listMcpTools() {
  return listCommands().map(commandToMcpTool)
}

export function getToolInput(input, fieldName) {
  const key = inputName(fieldName)
  if (input?.[key] !== undefined) return input[key]
  if (input?.[fieldName] !== undefined) return input[fieldName]
  for (const alias of ALIASES[key] || []) {
    if (input?.[alias] !== undefined) return input[alias]
  }
  return undefined
}
