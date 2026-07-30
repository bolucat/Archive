export const COMMAND_MANIFEST_VERSION = 9

const LARGE_OUTPUT_COMMANDS = new Set([
  'files list',
  'files walk',
  'files tree',
  'files stats',
  'files search',
])

const PROVIDER_REQUIREMENTS = {
  'files mkdir': { capability: 'mkdir' },
  'files rename-apply': { capability: 'batchRename' },
  'files move-apply': { capability: 'move' },
  'files trash-apply': { capability: 'trash' },
  'upload apply': { capability: 'uploadFile' },
  'files download': { capability: 'downloadFile' },
}

function arg(name, fields = {}) {
  return { name, ...fields }
}

function opt(name, fields = {}) {
  return { name, ...fields }
}

export const COMMAND_MANIFEST = [
  {
    group: 'auth',
    name: 'list',
    command: 'auth list',
    description: 'List configured cloud-drive accounts without secrets.',
    access: 'read',
    args: [],
    options: [opt('json', { type: 'boolean' })],
    output: 'AccountSummary[]',
  },
  {
    group: 'auth',
    name: 'default',
    command: 'auth default',
    description: 'Set the default account for one provider.',
    access: 'write',
    args: [arg('provider', { required: true, positional: true }), arg('account_id', { required: true, positional: true })],
    options: [opt('json', { type: 'boolean' })],
    output: 'AccountSummary',
  },
  {
    group: 'auth',
    name: 'import-token',
    command: 'auth import-token',
    description: 'Import a provider token into the standalone CLI auth store.',
    access: 'write',
    args: [],
    options: [
      opt('provider', { type: 'string', required: true }),
      opt('account', { type: 'string', required: true }),
      opt('token', { type: 'path', required: true }),
      opt('name', { type: 'string' }),
      opt('default', { type: 'boolean' }),
      opt('json', { type: 'boolean' }),
    ],
    output: 'AccountSummary',
  },
  {
    group: 'auth',
    name: 'login',
    command: 'auth login',
    description: 'Log in to the standalone CLI using QR or browser OAuth.',
    access: 'write',
    args: [arg('provider', { required: true, positional: true })],
    options: [
      opt('browser', { type: 'string' }),
      opt('redirect-uri', { type: 'string' }),
      opt('port', { type: 'number' }),
      opt('timeout-ms', { type: 'number' }),
      opt('json', { type: 'boolean' }),
    ],
    output: 'AccountSummary',
  },
  {
    group: 'settings',
    name: 'show',
    command: 'settings show',
    description: 'Show config directory, configured accounts, defaults, and supported providers.',
    access: 'read',
    args: [],
    options: [opt('json', { type: 'boolean' })],
    output: 'SettingsSummary',
  },
  {
    group: 'schema',
    name: 'commands',
    command: 'schema commands',
    description: 'Return the machine-readable command manifest.',
    access: 'read',
    args: [],
    options: [opt('group', { type: 'string' }), opt('json', { type: 'boolean' })],
    output: 'CommandManifest',
  },
  {
    group: 'schema',
    name: 'plans',
    command: 'schema plans',
    description: 'Return machine-readable schemas and examples for plan JSON files.',
    access: 'read',
    args: [],
    options: [opt('name', { type: 'string' }), opt('json', { type: 'boolean' })],
    output: 'PlanSchema[]',
  },
  {
    group: 'providers',
    name: 'capabilities',
    command: 'providers capabilities',
    description: 'List provider capability metadata.',
    access: 'read',
    args: [],
    options: [opt('json', { type: 'boolean' })],
    output: 'ProviderCapability[]',
  },
  {
    group: 'files',
    name: 'list',
    command: 'files list',
    description: 'List one cloud-drive directory.',
    access: 'read',
    args: [],
    options: [opt('provider', { type: 'string' }), opt('account', { type: 'string' }), opt('file-id', { type: 'string' }), opt('limit', { type: 'number' }), opt('cursor', { type: 'string' }), opt('drive-id', { type: 'string' }), opt('json', { type: 'boolean' })],
    output: 'FileItem[] | FileListPage',
  },
  {
    group: 'files',
    name: 'walk',
    command: 'files walk',
    description: 'Recursively walk a cloud-drive directory.',
    access: 'read',
    args: [],
    options: [opt('provider', { type: 'string' }), opt('account', { type: 'string' }), opt('file-id', { type: 'string' }), opt('drive-id', { type: 'string' }), opt('json', { type: 'boolean' })],
    output: 'FileItem[]',
  },
  {
    group: 'files',
    name: 'tree',
    command: 'files tree',
    description: 'Return a depth-limited tree summary.',
    access: 'read',
    args: [],
    options: [opt('provider', { type: 'string' }), opt('account', { type: 'string' }), opt('file-id', { type: 'string' }), opt('depth', { type: 'number' }), opt('drive-id', { type: 'string' }), opt('json', { type: 'boolean' })],
    output: 'TreeNode',
  },
  {
    group: 'files',
    name: 'stats',
    command: 'files stats',
    description: 'Aggregate size, count, category, and extension statistics for a directory.',
    access: 'read',
    args: [],
    options: [opt('provider', { type: 'string' }), opt('account', { type: 'string' }), opt('file-id', { type: 'string' }), opt('depth', { type: 'number' }), opt('drive-id', { type: 'string' }), opt('json', { type: 'boolean' })],
    output: 'DirectoryStats',
  },
  {
    group: 'files',
    name: 'info',
    command: 'files info',
    description: 'Return metadata for one file or folder.',
    access: 'read',
    args: [],
    options: [opt('file-id', { type: 'string', required: true }), opt('provider', { type: 'string' }), opt('account', { type: 'string' }), opt('drive-id', { type: 'string' }), opt('json', { type: 'boolean' })],
    output: 'FileItem',
  },
  {
    group: 'files',
    name: 'download',
    command: 'files download',
    description: 'Download one cloud-drive file to a local path.',
    access: 'read',
    args: [],
    options: [opt('file-id', { type: 'string', required: true }), opt('output', { type: 'path', required: true }), opt('provider', { type: 'string' }), opt('account', { type: 'string' }), opt('drive-id', { type: 'string' }), opt('json', { type: 'boolean' })],
    output: 'DownloadResult',
  },
  {
    group: 'files',
    name: 'search',
    command: 'files search',
    description: 'Search files by provider-side name or query support.',
    access: 'read',
    args: [],
    options: [opt('name', { type: 'string' }), opt('query', { type: 'string' }), opt('provider', { type: 'string' }), opt('account', { type: 'string' }), opt('limit', { type: 'number' }), opt('drive-id', { type: 'string' }), opt('json', { type: 'boolean' })],
    output: 'FileItem[]',
  },
  {
    group: 'files',
    name: 'mkdir',
    command: 'files mkdir',
    description: 'Create a folder.',
    access: 'write',
    args: [],
    options: [opt('name', { type: 'string', required: true }), opt('parent', { type: 'string' }), opt('provider', { type: 'string' }), opt('account', { type: 'string' }), opt('drive-id', { type: 'string' }), opt('json', { type: 'boolean' })],
    output: 'FileItem',
    requiresDryRun: false,
    undoable: false,
  },
  {
    group: 'files',
    name: 'rename-apply',
    command: 'files rename-apply',
    description: 'Validate or apply a rename plan.',
    access: 'write',
    args: [arg('plan', { type: 'path', required: true, positional: true })],
    options: [opt('current', { type: 'path' }), opt('dry-run', { type: 'boolean' }), opt('json', { type: 'boolean' })],
    output: 'RenameApplyResult',
    requiresDryRun: true,
    undoable: true,
  },
  {
    group: 'files',
    name: 'move-apply',
    command: 'files move-apply',
    description: 'Validate or apply a move plan.',
    access: 'write',
    args: [arg('plan', { type: 'path', required: true, positional: true })],
    options: [opt('dry-run', { type: 'boolean' }), opt('json', { type: 'boolean' })],
    output: 'MoveApplyResult',
    requiresDryRun: true,
    undoable: true,
  },
  {
    group: 'files',
    name: 'trash-apply',
    command: 'files trash-apply',
    description: 'Preview or apply a trash/delete plan. Defaults to dry-run; execution requires --apply.',
    access: 'write',
    args: [arg('plan', { type: 'path', required: true, positional: true })],
    options: [opt('apply', { type: 'boolean' }), opt('json', { type: 'boolean' })],
    output: 'TrashApplyResult',
    destructive: true,
    requiresDryRun: true,
    undoable: false,
  },
  {
    group: 'upload',
    name: 'plan',
    command: 'upload plan',
    description: 'Scan a local path and generate an upload plan.',
    access: 'read',
    args: [],
    options: [opt('local', { type: 'path', required: true }), opt('provider', { type: 'string' }), opt('account', { type: 'string' }), opt('remote-parent', { type: 'string' }), opt('output', { type: 'path' }), opt('json', { type: 'boolean' })],
    output: 'UploadPlan',
  },
  {
    group: 'upload',
    name: 'apply',
    command: 'upload apply',
    description: 'Validate or execute an upload plan.',
    access: 'write',
    args: [arg('plan', { type: 'path', required: true, positional: true })],
    options: [opt('dry-run', { type: 'boolean' }), opt('json', { type: 'boolean' })],
    output: 'UploadApplyResult',
    requiresDryRun: true,
    undoable: false,
  },
  {
    group: 'ops',
    name: 'list',
    command: 'ops list',
    description: 'List recorded operations.',
    access: 'read',
    args: [],
    options: [opt('json', { type: 'boolean' })],
    output: 'OperationSummary[]',
  },
  {
    group: 'ops',
    name: 'show',
    command: 'ops show',
    description: 'Show one recorded operation.',
    access: 'read',
    args: [arg('operation_id', { required: true, positional: true })],
    options: [opt('json', { type: 'boolean' })],
    output: 'OperationLog',
  },
  {
    group: 'ops',
    name: 'undo',
    command: 'ops undo',
    description: 'Preview or apply an undo plan for a supported operation.',
    access: 'write',
    args: [arg('operation_id', { required: true, positional: true })],
    options: [opt('dry-run', { type: 'boolean' }), opt('json', { type: 'boolean' })],
    output: 'UndoResult',
    requiresDryRun: true,
    undoable: false,
  },
]

function hasOption(command, name) {
  return (command.options || []).some((option) => option.name === name)
}

function examplesFor(command) {
  const c = command.command
  if (c === 'auth list') return ['clouddrive-cli auth list --format json']
  if (c === 'schema commands') return ['clouddrive-cli schema commands --format json']
  if (c === 'schema plans') return ['clouddrive-cli schema plans --format json']
  if (c === 'providers capabilities') return ['clouddrive-cli providers capabilities --format json']
  if (c === 'files list') return ['clouddrive-cli files list --provider aliyun --account default --file-id root --limit 100 --format json']
  if (c === 'files download') return ['clouddrive-cli files download --provider aliyun --account default --file-id <file-id> --output ./download.bin --format json']
  if (c === 'files walk') return ['clouddrive-cli files walk --provider aliyun --account default --file-id <folder-id> --output files.json --format json']
  if (c === 'files stats') return ['clouddrive-cli files stats --provider aliyun --account default --file-id root --depth 2 --output stats.json --format json']
  if (c === 'upload apply') return ['clouddrive-cli upload apply upload-plan.json --dry-run --rationale "User requested backup" --format json']
  return [`clouddrive-cli ${c} --format json`]
}

function decorateCommand(command) {
  const largeOutput = LARGE_OUTPUT_COMMANDS.has(command.command)
  const options = [...(command.options || [])]
  if (largeOutput && !hasOption(command, 'output')) options.splice(Math.max(0, options.length - 1), 0, opt('output', { type: 'path' }))
  if (command.requiresDryRun && !hasOption({ options }, 'confirm-apply')) options.splice(Math.max(0, options.length - 1), 0, opt('confirm-apply', { type: 'boolean' }))
  if (command.access === 'write' && !hasOption({ options }, 'rationale')) options.splice(Math.max(0, options.length - 1), 0, opt('rationale', { type: 'string' }))
  return {
    ...command,
    options,
    examples: command.examples || examplesFor(command),
    largeOutput,
    safety: {
      dryRunRequired: !!command.requiresDryRun,
      destructive: !!command.destructive,
      undoable: !!command.undoable,
    },
    ...(PROVIDER_REQUIREMENTS[command.command] ? { providerRequirements: PROVIDER_REQUIREMENTS[command.command] } : {}),
  }
}

export function listCommands({ group } = {}) {
  const commands = group ? COMMAND_MANIFEST.filter((command) => command.group === group) : COMMAND_MANIFEST
  return commands.map(decorateCommand)
}
