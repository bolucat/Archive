import { listCommands } from './commandManifest.mjs'

const JSON_OPTIONS = new Set(['json', 'format'])

const DEFAULTS = {
  account: 'default',
  provider: 'aliyun',
  style: 'jellyfin',
  limit: 100,
}

const DESCRIPTIONS = {
  account: 'Account id or "default".',
  analysis: 'Path to an organize analysis JSON file.',
  apply: 'Set true to execute a destructive trash/delete plan. Defaults to preview only.',
  browser: 'Browser command/name to open for OAuth login.',
  current: 'Optional current FileItem JSON path used to validate a rename plan.',
  default: 'Set the imported account as the default account for the provider.',
  depth: 'Maximum recursion depth.',
  'drive-id': 'Drive id. Uses provider/account default when omitted.',
  'dry-run': 'Preview without making changes. Defaults to true for commands that require dry-run first.',
  'file-id': 'File or folder id.',
  input: 'Path to an input JSON file.',
  cursor: 'Pagination cursor returned by the previous page.',
  local: 'Local file or directory path.',
  'max-chars': 'Maximum number of characters to read.',
  name: 'Name or display name, depending on command.',
  output: 'Optional output JSON path.',
  parent: 'Parent folder id.',
  path: 'Local document path.',
  'pdf-content-safety-off': 'Disable OpenDataLoader content safety filters: all, hidden-text, off-page, tiny, hidden-ocg.',
  'pdf-detect-strikethrough': 'Detect strikethrough text and preserve it in Markdown or HTML output.',
  'pdf-format': 'OpenDataLoader output format for PDF reads.',
  'pdf-html-page-separator': 'Separator inserted between pages in HTML output. Supports %page-number%.',
  'pdf-hybrid': 'OpenDataLoader hybrid backend: off, docling-fast, or hancom-ai.',
  'pdf-hybrid-fallback': 'Fallback to the Java pipeline if the hybrid backend errors.',
  'pdf-hybrid-hancom-ai-image-cache': 'Hancom AI page image cache backing: memory or disk.',
  'pdf-hybrid-hancom-ai-ocr-strategy': 'Hancom AI OCR strategy: off, auto, or force.',
  'pdf-hybrid-hancom-ai-regionlist-strategy': 'Hancom AI regionlist handling: table-first or list-only.',
  'pdf-hybrid-mode': 'Hybrid triage mode: auto or full.',
  'pdf-hybrid-timeout': 'Hybrid backend request timeout in milliseconds.',
  'pdf-hybrid-url': 'Hybrid backend server URL.',
  'pdf-image-dir': 'Directory for extracted images when image output is external.',
  'pdf-image-format': 'Extracted image format: png or jpeg.',
  'pdf-image-output': 'Image extraction mode: off, embedded, or external.',
  'pdf-include-header-footer': 'Include page headers and footers in output.',
  'pdf-keep-line-breaks': 'Preserve original PDF line breaks in extracted text.',
  'pdf-markdown-page-separator': 'Separator inserted between pages in Markdown output. Supports %page-number%.',
  'pdf-markdown-with-html': 'Allow HTML tags in Markdown for complex PDF structures such as tables.',
  'pdf-pages': 'PDF pages to extract, for example "1,3,5-7".',
  'pdf-password': 'Password for encrypted PDF files.',
  'pdf-reading-order': 'Reading order algorithm: off or xycut.',
  'pdf-replace-invalid-chars': 'Replacement character for invalid or unrecognized characters.',
  'pdf-sanitize': 'Sanitize sensitive values such as emails, phone numbers, IPs, credit cards, and URLs.',
  'pdf-table-method': 'Table detection method: default or cluster.',
  'pdf-text-page-separator': 'Separator inserted between pages in text output. Supports %page-number%.',
  'pdf-threads': 'Number of worker threads for OpenDataLoader per-page processing.',
  'pdf-to-stdout': 'Request OpenDataLoader stdout output. Useful only for single-format conversion.',
  'pdf-use-struct-tree': 'Use the PDF structure tree for reading order and semantic structure.',
  'pdf-verbose': 'Allow OpenDataLoader console logging instead of forcing quiet mode.',
  plan: 'Path to a plan JSON file. For MCP calls, a JSON plan object is also accepted and written to a temporary file.',
  provider: 'Provider id such as aliyun, cloud123, 115, 139, 189, quark, baidu, pikpak, onedrive, box, or dropbox.',
  query: 'Provider-specific raw search query.',
  rationale: 'Short reason for this write operation. Stored in results/logs for auditability.',
  'redirect-uri': 'OAuth redirect URI. Use a registered loopback URI when the provider requires one.',
  'remote-parent': 'Remote parent folder id for upload planning.',
  root: 'Root folder id where organized media structure should be created.',
  rules: 'Optional rules document path.',
  summary: 'Return or print a compact summary.',
  token: 'Path to a token JSON file.',
}

const ENUMS = {
  'pdf-format': ['json', 'text', 'html', 'pdf', 'markdown', 'tagged-pdf', 'json,markdown', 'json,html,pdf,markdown'],
  'pdf-hybrid': ['off', 'docling-fast', 'hancom-ai'],
  'pdf-hybrid-hancom-ai-image-cache': ['memory', 'disk'],
  'pdf-hybrid-hancom-ai-ocr-strategy': ['off', 'auto', 'force'],
  'pdf-hybrid-hancom-ai-regionlist-strategy': ['table-first', 'list-only'],
  'pdf-hybrid-mode': ['auto', 'full'],
  'pdf-image-format': ['png', 'jpeg'],
  'pdf-image-output': ['off', 'embedded', 'external'],
  'pdf-reading-order': ['off', 'xycut'],
  'pdf-table-method': ['default', 'cluster'],
  provider: ['aliyun', 'cloud123', '115', '139', '189', 'quark', 'baidu', 'pikpak', 'onedrive', 'box', 'dropbox'],
  style: ['jellyfin', 'emby', 'plex'],
}

const ALIASES = {
  drive_id: ['driveId'],
  file_id: ['fileId'],
  max_chars: ['maxChars'],
  operation_id: ['id', 'operationId'],
  pdf_content_safety_off: ['pdfContentSafetyOff'],
  pdf_detect_strikethrough: ['pdfDetectStrikethrough'],
  pdf_format: ['pdfFormat'],
  pdf_html_page_separator: ['pdfHtmlPageSeparator'],
  pdf_hybrid_fallback: ['pdfHybridFallback'],
  pdf_hybrid_hancom_ai_image_cache: ['pdfHybridHancomAiImageCache'],
  pdf_hybrid_hancom_ai_ocr_strategy: ['pdfHybridHancomAiOcrStrategy'],
  pdf_hybrid_hancom_ai_regionlist_strategy: ['pdfHybridHancomAiRegionlistStrategy'],
  pdf_hybrid_mode: ['pdfHybridMode'],
  pdf_hybrid_timeout: ['pdfHybridTimeout'],
  pdf_hybrid_url: ['pdfHybridUrl'],
  pdf_image_dir: ['pdfImageDir'],
  pdf_image_format: ['pdfImageFormat'],
  pdf_image_output: ['pdfImageOutput'],
  pdf_include_header_footer: ['pdfIncludeHeaderFooter'],
  pdf_keep_line_breaks: ['pdfKeepLineBreaks'],
  pdf_markdown_page_separator: ['pdfMarkdownPageSeparator'],
  pdf_markdown_with_html: ['pdfMarkdownWithHtml'],
  pdf_pages: ['pdfPages'],
  pdf_password: ['pdfPassword'],
  pdf_reading_order: ['pdfReadingOrder'],
  pdf_replace_invalid_chars: ['pdfReplaceInvalidChars'],
  pdf_sanitize: ['pdfSanitize'],
  pdf_table_method: ['pdfTableMethod'],
  pdf_text_page_separator: ['pdfTextPageSeparator'],
  pdf_threads: ['pdfThreads'],
  pdf_to_stdout: ['pdfToStdout'],
  pdf_use_struct_tree: ['pdfUseStructTree'],
  pdf_verbose: ['pdfVerbose'],
  redirect_uri: ['redirectUri'],
  remote_parent: ['remoteParent'],
  root: ['root_file_id', 'rootFileId'],
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
