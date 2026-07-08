export const COMMAND_MANIFEST_VERSION = 7

const LARGE_OUTPUT_COMMANDS = new Set([
  'files list',
  'files walk',
  'files tree',
  'files stats',
  'files search',
  'organize analyze',
])

const PROVIDER_REQUIREMENTS = {
  'files mkdir': { capability: 'mkdir' },
  'files rename-apply': { capability: 'batchRename' },
  'files move-apply': { capability: 'move' },
  'files trash-apply': { capability: 'trash' },
  'upload apply': { capability: 'uploadFile' },
  'files download': { capability: 'downloadFile' },
  'organize apply': { capability: 'mkdir/move/batchRename' },
}

function arg(name, fields = {}) {
  return { name, ...fields }
}

function opt(name, fields = {}) {
  return { name, ...fields }
}

const OPEN_DATALOADER_PDF_OPTIONS = [
  opt('pdf-format', { type: 'string' }),
  opt('pdf-password', { type: 'string' }),
  opt('pdf-content-safety-off', { type: 'string' }),
  opt('pdf-sanitize', { type: 'boolean' }),
  opt('pdf-keep-line-breaks', { type: 'boolean' }),
  opt('pdf-replace-invalid-chars', { type: 'string' }),
  opt('pdf-use-struct-tree', { type: 'boolean' }),
  opt('pdf-table-method', { type: 'string' }),
  opt('pdf-reading-order', { type: 'string' }),
  opt('pdf-markdown-page-separator', { type: 'string' }),
  opt('pdf-markdown-with-html', { type: 'boolean' }),
  opt('pdf-text-page-separator', { type: 'string' }),
  opt('pdf-html-page-separator', { type: 'string' }),
  opt('pdf-image-output', { type: 'string' }),
  opt('pdf-image-format', { type: 'string' }),
  opt('pdf-image-dir', { type: 'path' }),
  opt('pdf-pages', { type: 'string' }),
  opt('pdf-include-header-footer', { type: 'boolean' }),
  opt('pdf-detect-strikethrough', { type: 'boolean' }),
  opt('pdf-hybrid', { type: 'string' }),
  opt('pdf-hybrid-mode', { type: 'string' }),
  opt('pdf-hybrid-url', { type: 'string' }),
  opt('pdf-hybrid-timeout', { type: 'string' }),
  opt('pdf-hybrid-fallback', { type: 'boolean' }),
  opt('pdf-hybrid-hancom-ai-regionlist-strategy', { type: 'string' }),
  opt('pdf-hybrid-hancom-ai-ocr-strategy', { type: 'string' }),
  opt('pdf-hybrid-hancom-ai-image-cache', { type: 'string' }),
  opt('pdf-to-stdout', { type: 'boolean' }),
  opt('pdf-threads', { type: 'string' }),
  opt('pdf-verbose', { type: 'boolean' }),
]

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
    group: 'media',
    name: 'scan',
    command: 'media scan',
    description: 'Analyze FileItems and classify media files.',
    access: 'read',
    args: [],
    options: [opt('input', { type: 'path', required: true }), opt('json', { type: 'boolean' })],
    output: 'MediaScanReport',
  },
  {
    group: 'media',
    name: 'match',
    command: 'media match',
    description: 'Extract media naming metadata from FileItems.',
    access: 'read',
    args: [],
    options: [opt('input', { type: 'path', required: true }), opt('json', { type: 'boolean' })],
    output: 'MediaMatch[]',
  },
  {
    group: 'docs',
    name: 'read',
    command: 'docs read',
    description: 'Read a local document for AI context. PDF files are converted through OpenDataLoader when available.',
    access: 'read',
    args: [arg('path', { type: 'path', required: true, positional: true })],
    options: [
      opt('max-chars', { type: 'number' }),
      ...OPEN_DATALOADER_PDF_OPTIONS,
      opt('json', { type: 'boolean' }),
    ],
    output: 'DocumentContext',
  },
  {
    group: 'docs',
    name: 'convert',
    command: 'docs convert',
    description: 'Convert PDF files or folders through the full OpenDataLoader PDF option surface.',
    access: 'read',
    args: [arg('path', { type: 'path', required: true, positional: true })],
    options: [
      opt('output', { type: 'path', required: true }),
      ...OPEN_DATALOADER_PDF_OPTIONS,
      opt('json', { type: 'boolean' }),
    ],
    output: 'OpenDataLoaderConvertResult',
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
    group: 'organize',
    name: 'analyze',
    command: 'organize analyze',
    description: 'Analyze a cloud-drive directory or FileItem JSON input for organization.',
    access: 'read',
    args: [],
    options: [opt('input', { type: 'path' }), opt('provider', { type: 'string' }), opt('account', { type: 'string' }), opt('file-id', { type: 'string' }), opt('depth', { type: 'number' }), opt('output', { type: 'path' }), opt('summary', { type: 'boolean' }), opt('json', { type: 'boolean' })],
    output: 'OrganizeAnalysis',
  },
  {
    group: 'organize',
    name: 'plan',
    command: 'organize plan',
    description: 'Generate a cloud-drive organization plan.',
    access: 'read',
    args: [],
    options: [opt('analysis', { type: 'path', required: true }), opt('rules', { type: 'path' }), opt('output', { type: 'path' }), opt('summary', { type: 'boolean' }), opt('json', { type: 'boolean' })],
    output: 'OrganizePlan',
  },
  {
    group: 'organize',
    name: 'apply',
    command: 'organize apply',
    description: 'Validate or execute a cloud-drive organization plan.',
    access: 'write',
    args: [arg('plan', { type: 'path', required: true, positional: true })],
    options: [opt('dry-run', { type: 'boolean' }), opt('summary', { type: 'boolean' }), opt('json', { type: 'boolean' })],
    output: 'OrganizeApplyResult',
    requiresDryRun: true,
    undoable: true,
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
  if (c === 'providers capabilities') return ['clouddrive-cli providers capabilities --format json']
  if (c === 'files list') return ['clouddrive-cli files list --provider aliyun --account default --file-id root --limit 100 --format json']
  if (c === 'files download') return ['clouddrive-cli files download --provider aliyun --account default --file-id <file-id> --output ./download.bin --format json']
  if (c === 'files walk') return ['clouddrive-cli files walk --provider aliyun --account default --file-id <folder-id> --output files.json --format json']
  if (c === 'files stats') return ['clouddrive-cli files stats --provider aliyun --account default --file-id root --depth 2 --output stats.json --format json']
  if (c === 'docs read') return ['clouddrive-cli docs read ./rules.pdf --pdf-format markdown --pdf-pages 1-3 --format json']
  if (c === 'docs convert') return ['clouddrive-cli docs convert ./pdf-folder --output ./out --pdf-format json,html,pdf,markdown,tagged-pdf --format json']
  if (c === 'upload apply') return ['clouddrive-cli upload apply upload-plan.json --dry-run --rationale "User requested backup" --format json']
  return [`clouddrive-cli ${c} --format json`]
}

function decorateCommand(command) {
  const largeOutput = LARGE_OUTPUT_COMMANDS.has(command.command)
  const options = [...(command.options || [])]
  if (largeOutput && !hasOption(command, 'output')) options.splice(Math.max(0, options.length - 1), 0, opt('output', { type: 'path' }))
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
