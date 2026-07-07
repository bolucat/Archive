import { readdir, stat } from 'node:fs/promises'
import { basename, join, posix, relative, resolve, sep } from 'node:path'

function normalizeRelativePath(path) {
  return path.split(sep).join('/')
}

function hasText(value) {
  return typeof value === 'string' && value.trim().length > 0
}

function hasString(value) {
  return typeof value === 'string'
}

async function scanLocalPath(root, current = root) {
  const info = await stat(current)
  if (info.isDirectory()) {
    const entries = await readdir(current, { withFileTypes: true })
    const children = []
    const sorted = entries
      .filter((entry) => entry.name !== '.DS_Store')
      .sort((a, b) => {
        if (a.isDirectory() && !b.isDirectory()) return -1
        if (!a.isDirectory() && b.isDirectory()) return 1
        return a.name.localeCompare(b.name)
      })
    for (const entry of sorted) {
      children.push(...await scanLocalPath(root, join(current, entry.name)))
    }
    if (current === root) return children
    return [{
      type: 'folder',
      local_path: current,
      relative_path: normalizeRelativePath(relative(root, current)),
      target_name: basename(current),
    }, ...children]
  }
  if (!info.isFile()) return []
  return [{
    type: 'file',
    local_path: current,
    relative_path: normalizeRelativePath(relative(root, current)),
    target_name: basename(current),
    size: info.size,
    updated_at: info.mtime.toISOString(),
  }]
}

export async function createUploadPlanFromLocalPath({
  localPath,
  provider = 'aliyun',
  accountId = 'default',
  remoteParentFileId = 'root',
  conflict = 'skip',
} = {}) {
  if (!hasText(localPath)) throw new Error('--local is required')
  const localRoot = resolve(localPath)
  const items = await scanLocalPath(localRoot)
  return {
    version: 1,
    operation: 'upload',
    provider,
    account_id: accountId,
    created_at: new Date().toISOString(),
    local_root: localRoot,
    remote_parent_file_id: remoteParentFileId,
    conflict,
    items,
  }
}

export function validateUploadPlan(plan) {
  const errors = []
  if (!plan || typeof plan !== 'object') return { ok: false, errors: ['plan must be an object'], itemCount: 0 }
  if (plan.version !== 1) errors.push('version must be 1')
  if (plan.operation !== 'upload') errors.push('operation must be upload')
  if (!hasText(plan.provider)) errors.push('provider is required')
  if (!hasText(plan.account_id)) errors.push('account_id is required')
  if (!hasText(plan.local_root)) errors.push('local_root is required')
  if (!hasText(plan.remote_parent_file_id)) errors.push('remote_parent_file_id is required')
  if (!Array.isArray(plan.items)) {
    errors.push('items must be an array')
  } else {
    plan.items.forEach((item, index) => {
      if (item?.type !== 'file' && item?.type !== 'folder') errors.push(`items[${index}].type must be file or folder`)
      if (!hasString(item?.relative_path)) errors.push(`items[${index}].relative_path is required`)
      if (!hasText(item?.target_name)) errors.push(`items[${index}].target_name is required`)
      if (item?.type === 'file' && !hasText(item?.local_path)) errors.push(`items[${index}].local_path is required`)
    })
  }
  return { ok: errors.length === 0, errors, itemCount: Array.isArray(plan.items) ? plan.items.length : 0 }
}

export function dryRunUploadPlan(plan) {
  const validation = validateUploadPlan(plan)
  const errors = validation.errors.map((message) => ({ code: 'invalid_upload_plan', message }))
  const items = Array.isArray(plan?.items) ? plan.items : []
  const files = items.filter((item) => item.type === 'file')
  const folders = items.filter((item) => item.type === 'folder')
  return {
    ok: errors.length === 0,
    provider: plan?.provider,
    account_id: plan?.account_id,
    remote_parent_file_id: plan?.remote_parent_file_id,
    fileCount: files.length,
    folderCount: folders.length,
    totalBytes: files.reduce((sum, item) => sum + (Number(item.size) || 0), 0),
    items: errors.length === 0 ? items.map((item) => ({
      type: item.type,
      relative_path: item.relative_path,
      target_name: item.target_name,
      size: item.size || 0,
    })) : [],
    errors,
  }
}

function parentRelativePath(relativePath) {
  if (!relativePath) return ''
  const parent = posix.dirname(relativePath)
  return parent === '.' ? '' : parent
}

function remoteRefForCreatedFolder(folder, fallbackName, fallbackParent) {
  return {
    id: folder?.fileId || folder?.file_id || folder?.id || '',
    path: folder?.path || (fallbackParent.path ? `${fallbackParent.path}/${fallbackName}`.replace(/\/+/g, '/') : ''),
  }
}

export async function executeUploadPlan(plan, { provider, token, driveId } = {}) {
  const dryRun = dryRunUploadPlan(plan)
  if (!dryRun.ok) return dryRun
  if (!provider?.files?.uploadFile) {
    return {
      ok: false,
      provider: plan.provider,
      account_id: plan.account_id,
      remote_parent_file_id: plan.remote_parent_file_id,
      succeeded: 0,
      failed: dryRun.fileCount + dryRun.folderCount,
      fileCount: dryRun.fileCount,
      folderCount: dryRun.folderCount,
      totalBytes: dryRun.totalBytes,
      results: [],
      errors: [{ code: 'upload_adapter_missing', message: `Provider "${plan.provider}" upload adapter is not wired` }],
    }
  }

  const refs = new Map()
  refs.set('', { id: plan.remote_parent_file_id, path: plan.remote_parent_file_id })
  const results = []

  for (const item of plan.items) {
    const parentRel = parentRelativePath(item.relative_path)
    const parentRef = refs.get(parentRel)
    if (!parentRef) {
      results.push({
        type: item.type,
        relative_path: item.relative_path,
        target_name: item.target_name,
        status: 'error',
        code: 'missing_parent',
        message: `Parent folder was not created: ${parentRel}`,
      })
      continue
    }

    try {
      if (item.type === 'folder') {
        const created = await provider.files.mkdir({
          token,
          driveId,
          parentId: parentRef.id,
          parentPath: parentRef.path,
          name: item.target_name,
          conflict: plan.conflict,
        })
        const ref = remoteRefForCreatedFolder(created, item.target_name, parentRef)
        refs.set(item.relative_path, ref)
        results.push({
          type: 'folder',
          relative_path: item.relative_path,
          target_name: item.target_name,
          fileId: ref.id,
          path: ref.path,
          status: 'success',
        })
        continue
      }

      const uploaded = await provider.files.uploadFile({
        token,
        driveId,
        parentId: parentRef.id,
        parentPath: parentRef.path,
        localPath: item.local_path,
        name: item.target_name,
        size: Number(item.size) || 0,
        conflict: plan.conflict,
        relativePath: item.relative_path,
      })
      results.push({
        type: 'file',
        relative_path: item.relative_path,
        target_name: item.target_name,
        size: Number(item.size) || 0,
        fileId: uploaded?.fileId || uploaded?.file_id || uploaded?.id || '',
        path: uploaded?.path,
        rapid: !!(uploaded?.rapid || uploaded?.isRapid),
        status: 'success',
      })
    } catch (e) {
      results.push({
        type: item.type,
        relative_path: item.relative_path,
        target_name: item.target_name,
        size: Number(item.size) || 0,
        status: 'error',
        code: e?.code || 'ERR_UPLOAD',
        message: e?.message || 'Upload failed',
      })
    }
  }

  const succeeded = results.filter((item) => item.status === 'success').length
  const failed = results.length - succeeded
  return {
    ok: failed === 0,
    provider: plan.provider,
    account_id: plan.account_id,
    remote_parent_file_id: plan.remote_parent_file_id,
    succeeded,
    failed,
    fileCount: dryRun.fileCount,
    folderCount: dryRun.folderCount,
    totalBytes: dryRun.totalBytes,
    results,
    errors: results.filter((item) => item.status !== 'success').map((item) => ({
      code: item.code || 'ERR_UPLOAD',
      message: item.message || `Failed: ${item.relative_path}`,
      relative_path: item.relative_path,
    })),
  }
}
