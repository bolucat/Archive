import { readFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { randomBytes } from 'node:crypto'
import type { MediaAcquisitionCandidate, MediaAcquisitionRunView } from '@shared/types/mediaAcquisition'
import { getMediaAcquisitionCandidateLocator, getMediaAcquisitionRun } from './MediaAcquisitionService'
import { assertMediaAcquisitionV1TransferTicket, type MediaAcquisitionTransferTicket } from './MediaAcquisitionV1Bridge'

type CliTokenAccount = { provider?: string; accountId?: string; token?: { access_token?: string; device_id?: string } }
type ProviderTransferResult = { taskId?: string; fileId?: string; activity: string }
export type ProviderShareImportResult = { status: 'success' | 'async'; fileCount: number; activity: string }
export type ProviderTransferStatus = { progress: number; completed: boolean; failed: boolean; message?: string; error?: string }

const PIKPAK_API_HOST = 'https://api-drive.mypikpak.com'
const GUANGYA_API_URL = 'https://api.guangyapan.com'
const GUANGYA_WEB_URL = 'https://www.guangyapan.com'
const GUANGYA_USER_AGENT = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36'

/**
 * The only main-process entry point that creates media-agent provider tasks.
 * Candidate locators and cloud tokens stay in the main process; the renderer
 * receives only the opaque provider result needed for the task ledger.
 */
export async function submitMediaAcquisitionProviderTransfer(runId: string, candidateId: string, parentId: string, ticket?: MediaAcquisitionTransferTicket | null): Promise<ProviderTransferResult> {
  const run = getMediaAcquisitionRun(runId)
  const candidate = run?.candidates.find(item => item.id === candidateId)
  if (!run || !candidate) throw new Error('媒体获取任务或候选资源不存在')
  if (!['magnet', 'http'].includes(candidate.kind)) throw new Error('该候选不支持离线下载提交')
  if (ticket) assertMediaAcquisitionV1TransferTicket(ticket, runId, candidateId)
  const source = getMediaAcquisitionCandidateLocator(runId, candidateId)
  if (!source?.locator) throw new Error('候选资源链接不可用')
  if (candidate.kind === 'magnet' && !source.locator.startsWith('magnet:?')) throw new Error('无效的磁力链接')
  if (candidate.kind === 'http' && !/^https?:\/\//i.test(source.locator)) throw new Error('无效的 HTTP/HTTPS 链接')
  return createOfflineTask(run, candidate, source.locator, parentId)
}

/** Imports the top-level items of a share without disclosing the share URL, password, or account token to the renderer. */
export async function submitMediaAcquisitionProviderShareImport(runId: string, candidateId: string, parentId: string, ticket?: MediaAcquisitionTransferTicket | null): Promise<ProviderShareImportResult> {
  const run = getMediaAcquisitionRun(runId)
  const candidate = run?.candidates.find(item => item.id === candidateId)
  if (!run || !candidate || candidate.kind !== 'share') throw new Error('媒体分享候选不存在')
  if (ticket) assertMediaAcquisitionV1TransferTicket(ticket, runId, candidateId)
  const source = getMediaAcquisitionCandidateLocator(runId, candidateId)
  if (!source?.locator) throw new Error('候选分享链接不可用')
  const platform = parseSharePlatform(source.locator)
  if (!platform) throw new Error('无法识别分享链接')
  if (platform !== normalizePlatform(run.target.targetPlatform)) throw new Error(`分享链接属于 ${platform}，无法导入到 ${run.target.targetPlatform}`)
  if (platform === 'aliyun') return importAliyunShare(run, source.locator, source.password || '', parentId)
  if (platform === 'quark') return importQuarkShare(run, source.locator, source.password || '', parentId)
  if (platform === 'guangya') return importGuangyaShare(run, source.locator, source.password || '', parentId)
  return importPikPakShare(run, source.locator, source.password || '')
}

/** Used by the approved media workflow for ASSRT sidecars; provider creation still stays in main. */
export async function submitMediaAcquisitionExternalUrl(runId: string, parentId: string, url: string, fileName: string): Promise<ProviderTransferResult> {
  const run = getMediaAcquisitionRun(runId)
  if (!run) throw new Error('媒体获取任务不存在')
  if (!/^https?:\/\//i.test(url)) throw new Error('无效的 HTTP/HTTPS 链接')
  return createOfflineTask(run, { kind: 'http', title: fileName } as MediaAcquisitionCandidate, url, parentId)
}

/**
 * Reads the state of an offline task in the main process. This deliberately
 * accepts a run id rather than provider credentials, so the renderer never
 * needs a provider-specific polling client.
 */
export async function getMediaAcquisitionProviderTransferStatus(runId: string, taskId?: string, fileId?: string): Promise<ProviderTransferStatus> {
  const run = getMediaAcquisitionRun(runId)
  if (!run) throw new Error('媒体获取任务不存在')
  if (!taskId && fileId) return { progress: 100, completed: true, failed: false }
  if (!taskId) return { progress: 100, completed: true, failed: false, message: '正在核对分享转存结果' }
  const platform = normalizePlatform(run.target.targetPlatform)
  const token = await readProviderToken(platform, run.target.targetUserId)
  if (platform === '115') return get115OfflineTaskStatus(token.accessToken, taskId)
  if (platform === 'cloud123') return get123OfflineTaskStatus(token.accessToken, taskId)
  if (platform === 'pikpak') return getPikPakOfflineTaskStatus(token, taskId, fileId)
  if (platform === 'guangya') return getGuangyaOfflineTaskStatus(token, taskId)
  return { progress: 0, completed: false, failed: true, message: `${run.target.targetPlatform} 暂无离线任务查询接口` }
}

async function createOfflineTask(run: MediaAcquisitionRunView, candidate: MediaAcquisitionCandidate, locator: string, parentId: string): Promise<ProviderTransferResult> {
  const platform = normalizePlatform(run.target.targetPlatform)
  if (candidate.kind === 'magnet' && !['115', 'guangya', 'pikpak'].includes(platform)) throw new Error(`${run.target.targetPlatform} 当前不支持磁力离线下载`)
  if (candidate.kind === 'http' && !['115', 'guangya', 'pikpak', 'cloud123'].includes(platform)) throw new Error(`${run.target.targetPlatform} 当前不支持 HTTP 外链离线下载`)
  const token = await readProviderToken(platform, run.target.targetUserId)
  if (platform === '115') return create115OfflineTask(token.accessToken, locator, parentId, candidate.kind)
  if (platform === 'cloud123') return create123OfflineTask(token.accessToken, locator, candidate.title, parentId)
  if (platform === 'pikpak') return createPikPakOfflineTask(token, locator, candidate.title, parentId)
  return createGuangyaOfflineTask(token, locator, candidate.title, parentId)
}

async function readProviderToken(platform: string, userId: string): Promise<{ accessToken: string; deviceId: string }> {
  const raw = await readFile(join(homedir(), '.clouddrive-cli', 'tokens.json'), 'utf8').catch(() => '')
  const accounts = raw ? JSON.parse(raw).accounts as CliTokenAccount[] : []
  const aliases = platform === 'cloud123' ? new Set(['cloud123', '123']) : new Set([platform])
  const accountIds = platform === 'aliyun' ? new Set([userId, `aliyun_${userId}`]) : new Set([userId])
  const account = accounts.find(item => aliases.has(String(item.provider || '').toLowerCase()) && accountIds.has(String(item.accountId || '')))
  const accessToken = String(account?.token?.access_token || '')
  if (!accessToken) throw new Error(`未找到 ${platform} 账号授权；请在账号设置中重新登录或导出 CLI 账号`)
  return { accessToken, deviceId: String(account?.token?.device_id || '') }
}

async function create115OfflineTask(accessToken: string, locator: string, parentId: string, kind: MediaAcquisitionCandidate['kind']): Promise<ProviderTransferResult> {
  const body = new URLSearchParams({ urls: locator })
  const normalizedParentId = parentId === 'drive115' || parentId.includes('root') ? '0' : parentId
  if (normalizedParentId) body.set('wp_path_id', normalizedParentId)
  const data = await requestJson('https://proapi.115.com/open/offline/add_task_urls', {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/x-www-form-urlencoded' },
    body
  }, '115 云下载请求失败')
  if (data?.code !== 0 || data?.state === false) throw new Error(data?.message || '115 云下载请求失败')
  const taskIds = (Array.isArray(data?.data) ? data.data : []).filter((item: any) => item?.state !== false && item?.code === 0).map((item: any) => String(item?.info_hash || '')).filter(Boolean)
  if (!taskIds.length) throw new Error(data?.message || '115 云下载任务未创建')
  return { taskId: taskIds[0], activity: `已在主进程创建 115 ${kind === 'magnet' ? '磁力' : 'HTTP'} 云下载任务（${taskIds.length} 项），等待网盘完成` }
}

async function create123OfflineTask(accessToken: string, locator: string, title: string, parentId: string): Promise<ProviderTransferResult> {
  const body: Record<string, unknown> = { url: locator, fileName: title }
  if (parentId && parentId !== 'cloud123' && !parentId.includes('root') && Number.isFinite(Number(parentId))) body.dirID = Number(parentId)
  const data = await requestJson('https://open-api.123pan.com/api/v1/offline/download', {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json', Platform: 'open_platform' },
    body: JSON.stringify(body)
  }, '创建 123 云盘离线下载任务失败')
  if (data?.code !== 0) throw new Error(data?.message || '创建 123 云盘离线下载任务失败')
  const taskId = String(data?.data?.taskID || data?.data?.taskId || '')
  if (!taskId) throw new Error('123 云盘未返回有效任务 ID')
  return { taskId, activity: '已在主进程创建 123 云盘 HTTP 离线下载任务，等待网盘完成' }
}

async function createPikPakOfflineTask(token: { accessToken: string; deviceId: string }, locator: string, title: string, parentId: string): Promise<ProviderTransferResult> {
  const body: Record<string, unknown> = { kind: 'drive#file', name: title || undefined, upload_type: 'UPLOAD_TYPE_URL', url: { url: locator } }
  const normalizedParentId = parentId === 'pikpak' || parentId.includes('root') ? '' : parentId
  if (normalizedParentId) {
    body.parent_id = normalizedParentId
    body.folder_type = ''
  } else body.folder_type = 'DOWNLOAD'
  const data = await requestJson(`${PIKPAK_API_HOST}/drive/v1/files`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${token.accessToken}`, 'Content-Type': 'application/json; charset=utf-8', 'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36', ...(token.deviceId ? { 'X-Device-Id': token.deviceId } : {}) },
    body: JSON.stringify(body)
  }, '创建 PikPak 离线下载任务失败')
  if (data?.error) throw new Error(data?.error_description || data?.message || data.error)
  const taskId = String(data?.task?.id || data?.task_id || data?.upload_task_id || '')
  const fileId = String(data?.file?.id || data?.file_id || data?.id || '')
  if (!taskId && !fileId) throw new Error('创建 PikPak 离线下载任务失败')
  return { taskId: taskId || undefined, fileId: fileId || undefined, activity: '已在主进程创建 PikPak 离线下载任务，等待网盘完成' }
}

async function createGuangyaOfflineTask(token: { accessToken: string; deviceId: string }, locator: string, title: string, parentId: string): Promise<ProviderTransferResult> {
  const data = await requestJson(`${GUANGYA_API_URL}/cloudcollection/v1/create_task`, {
    method: 'POST',
    headers: guangyaHeaders(token),
    body: JSON.stringify({ url: locator, parentId: parentId === 'guangya' || parentId === 'guangya_root' || parentId === '0' || parentId === '/' ? '' : parentId, newName: title || 'offline' })
  }, '创建光鸭云盘离线下载任务失败')
  const task = data?.data || data || {}
  const taskId = String(task?.taskId || task?.id || task?.task_id || '')
  const fileId = String(task?.fileId || task?.file_id || task?.resId || '')
  if (!taskId && !fileId) throw new Error(data?.message || data?.msg || '创建光鸭云盘离线下载任务失败')
  return { taskId: taskId || undefined, fileId: fileId || undefined, activity: '已在主进程创建光鸭云盘离线下载任务，等待网盘完成' }
}

async function get115OfflineTaskStatus(accessToken: string, taskId: string): Promise<ProviderTransferStatus> {
  const tasks: any[] = []
  let page = 1
  let pageCount = 1
  do {
    const data = await requestJson(`https://proapi.115.com/open/offline/get_task_list?page=${page}`, { headers: { Authorization: `Bearer ${accessToken}` } }, '获取 115 云下载进度失败')
    if (data?.code !== 0 || data?.state === false) throw new Error(data?.message || '获取 115 云下载进度失败')
    tasks.push(...(Array.isArray(data?.data?.tasks) ? data.data.tasks : []))
    pageCount = Math.max(1, Number(data?.data?.page_count) || 1)
    page += 1
  } while (page <= pageCount)
  const task = tasks.find(item => String(item?.info_hash || '') === String(taskId))
  if (!task) return { progress: 0, completed: false, failed: false, error: '115 云下载任务不存在' }
  const progress = clampProgress(task?.percentDone)
  const status = Number(task?.status) || 0
  return { progress, completed: status === 2 || progress >= 100, failed: status < 0, message: String(task?.name || '') }
}

async function get123OfflineTaskStatus(accessToken: string, taskId: string): Promise<ProviderTransferStatus> {
  if (!Number.isFinite(Number(taskId))) return { progress: 0, completed: false, failed: false, error: '任务 ID 无效' }
  const data = await requestJson(`https://open-api.123pan.com/api/v1/offline/download/process?taskID=${encodeURIComponent(taskId)}`, { headers: { Authorization: `Bearer ${accessToken}`, Platform: 'open_platform' } }, '获取 123 云盘离线下载进度失败')
  if (data?.code !== 0) throw new Error(data?.message || '获取 123 云盘离线下载进度失败')
  const progress = clampProgress(data?.data?.process)
  const status = Number(data?.data?.status) || 0
  return { progress, completed: status === 2 || progress >= 100, failed: status < 0 }
}

async function getPikPakOfflineTaskStatus(token: { accessToken: string; deviceId: string }, taskId: string, fileId?: string): Promise<ProviderTransferStatus> {
  const params = new URLSearchParams({ type: 'offline', thumbnail_size: 'SIZE_SMALL', limit: '100', with: 'reference_resource', filters: JSON.stringify({ phase: { in: 'PHASE_TYPE_PENDING,PHASE_TYPE_RUNNING,PHASE_TYPE_COMPLETE,PHASE_TYPE_ERROR' } }) })
  const data = await requestJson(`${PIKPAK_API_HOST}/drive/v1/tasks?${params}`, { headers: pikpakHeaders(token) }, '获取 PikPak 离线下载进度失败')
  const task = (Array.isArray(data?.tasks) ? data.tasks : []).find((item: any) => String(item?.id || item?.task_id || '') === String(taskId))
  if (task) {
    const phase = String(task?.phase || task?.status || '')
    const progress = clampProgress(task?.progress ?? task?.params?.progress ?? task?.progress_percent)
    return { progress: phase === 'PHASE_TYPE_COMPLETE' ? 100 : progress, completed: phase === 'PHASE_TYPE_COMPLETE', failed: phase === 'PHASE_TYPE_ERROR', message: String(task?.message || task?.error || '') || undefined }
  }
  if (!fileId) return { progress: 0, completed: false, failed: false }
  const file = await requestJson(`${PIKPAK_API_HOST}/drive/v1/files/${encodeURIComponent(fileId)}?thumbnail_size=SIZE_LARGE`, { headers: pikpakHeaders(token) }, '获取 PikPak 离线下载文件失败').catch(() => undefined)
  return file?.id ? { progress: 100, completed: true, failed: false } : { progress: 0, completed: false, failed: false }
}

async function getGuangyaOfflineTaskStatus(token: { accessToken: string; deviceId: string }, taskId: string): Promise<ProviderTransferStatus> {
  const data = await requestJson(`${GUANGYA_API_URL}/cloudcollection/v1/list_task`, { method: 'POST', headers: guangyaHeaders(token), body: JSON.stringify({ taskIds: [taskId] }) }, '获取光鸭云盘离线下载进度失败')
  const body = data?.data || data || {}
  const items = body?.list || body?.items || body?.records || body?.content || []
  const task = Array.isArray(items) ? items.find((item: any) => String(item?.taskId || item?.id || item?.task_id || '') === String(taskId)) : undefined
  if (!task) return { progress: 0, completed: false, failed: false }
  const status = Number(task?.status ?? task?.taskStatus ?? 0)
  const progress = clampProgress(task?.process ?? task?.progress ?? task?.percent)
  return { progress, completed: status === 2 || progress >= 100, failed: status === 3 || status === 4 }
}

function clampProgress(value: unknown): number {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return 0
  const percent = numeric > 0 && numeric <= 1 ? numeric * 100 : numeric
  return Math.max(0, Math.min(100, Math.floor(percent)))
}

function guangyaHeaders(token: { accessToken: string; deviceId: string }): Record<string, string> {
  const hex = (size: number) => randomBytes(size).toString('hex')
  return { accept: 'application/json, text/plain, */*', ...(token.accessToken ? { authorization: `Bearer ${token.accessToken}` } : {}), 'content-type': 'application/json', did: token.deviceId || hex(16), dt: '4', origin: GUANGYA_WEB_URL, referer: `${GUANGYA_WEB_URL}/`, traceparent: `00-${hex(16)}-${hex(8)}-01`, 'user-agent': GUANGYA_USER_AGENT }
}

async function requestJson(url: string, init: RequestInit, fallback: string): Promise<any> {
  const response = await fetch(url, init)
  const data = await response.json().catch(() => undefined)
  if (!response.ok) throw new Error(data?.message || data?.msg || data?.error_description || data?.error || `${fallback}（HTTP ${response.status}）`)
  return data
}

function normalizePlatform(value: string): string {
  const platform = String(value || '').toLowerCase()
  if (platform === 'ali' || platform === 'alipan') return 'aliyun'
  if (platform === '123' || platform === '123pan') return 'cloud123'
  return platform
}

function parseSharePlatform(locator: string): 'aliyun' | 'quark' | 'guangya' | 'pikpak' | null {
  if (/(?:aliyundrive|alipan)\.com\/s\/[A-Za-z0-9_-]+/i.test(locator)) return 'aliyun'
  if (/(?:pan\.quark\.cn\/s\/|quark:\/\/share\/)[A-Za-z0-9_-]+/i.test(locator)) return 'quark'
  if (/guangyapan\.com\/s\/[A-Za-z0-9_-]+/i.test(locator)) return 'guangya'
  if (/(?:mypikpak|pikpak)\.com\/s\/[A-Za-z0-9_-]+/i.test(locator)) return 'pikpak'
  return null
}

function shareId(locator: string, platform: string): string {
  const patterns: Record<string, RegExp> = {
    aliyun: /(?:aliyundrive|alipan)\.com\/s\/([A-Za-z0-9_-]+)/i,
    quark: /(?:pan\.quark\.cn\/s\/|quark:\/\/share\/)([A-Za-z0-9_-]+)/i,
    guangya: /guangyapan\.com\/s\/([A-Za-z0-9_-]+)/i,
    pikpak: /(?:mypikpak|pikpak)\.com\/s\/([A-Za-z0-9_-]+)/i
  }
  return locator.match(patterns[platform])?.[1] || ''
}

function sharePassword(locator: string, explicit: string, platform: string): string {
  if (explicit) return explicit
  if (platform === 'pikpak') return locator.match(/[?&#]pass_code=([A-Za-z0-9]+)/i)?.[1] || ''
  return locator.match(/[?&#](?:pwd|password|share_pwd)=([A-Za-z0-9]+)/i)?.[1] || locator.match(/(?:提取码|密码|password|passcode|pwd|code)[:：\s]*([A-Za-z0-9]+)/i)?.[1] || ''
}

async function importAliyunShare(run: MediaAcquisitionRunView, locator: string, password: string, parentId: string): Promise<ProviderShareImportResult> {
  const id = shareId(locator, 'aliyun')
  const tokenReply = await requestJson('https://api.aliyundrive.com/v2/share_link/get_share_token', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ share_id: id, share_pwd: sharePassword(locator, password, 'aliyun') }) }, '获取阿里云盘分享凭证失败')
  const shareToken = String(tokenReply?.share_token || '')
  if (!shareToken) throw new Error(tokenReply?.code || '获取阿里云盘分享凭证失败')
  const files = await listAliyunShareFiles(id, shareToken)
  if (!files.length) throw new Error('分享中没有可导入的文件')
  const target = await readProviderToken('aliyun', run.target.targetUserId)
  let asyncSubmitted = false
  for (const fileIds of chunks(files.map(item => String(item.file_id || '')).filter(Boolean), 100)) {
    const requests = fileIds.map(fileId => ({ body: { share_id: id, file_id_list: [''], file_id: fileId, to_drive_id: run.target.targetDriveId, to_parent_file_id: parentId.includes('root') ? 'root' : parentId, auto_rename: true }, headers: { 'Content-Type': 'application/json' }, id: fileId, method: 'POST', url: '/file/copy' }))
    const reply = await requestJson('https://api.aliyundrive.com/adrive/v4/batch', { method: 'POST', headers: { Authorization: `Bearer ${target.accessToken}`, 'Content-Type': 'application/json', 'x-share-token': shareToken }, body: JSON.stringify({ requests, resource: 'file' }) }, '阿里云盘分享转存失败')
    const responses = Array.isArray(reply?.responses) ? reply.responses : []
    const failure = responses.find((item: any) => Number(item?.status) >= 400 || item?.body?.code)
    if (failure) throw new Error(failure?.body?.message || failure?.body?.code || '阿里云盘分享转存失败')
    asyncSubmitted ||= responses.some((item: any) => !!item?.body?.async_task_id)
  }
  return { status: asyncSubmitted ? 'async' : 'success', fileCount: files.length, activity: asyncSubmitted ? '已在主进程提交异步阿里云盘分享转存，等待网盘完成' : '已在主进程提交阿里云盘分享转存，正在核对入库目录' }
}

async function listAliyunShareFiles(id: string, shareToken: string): Promise<any[]> {
  const items: any[] = []
  let marker = ''
  do {
    const url = 'https://api.aliyundrive.com/adrive/v3/file/list?jsonmask=next_marker%2Citems(category%2Ccreated_at%2Cdrive_id%2Cfile_extension%2Cfile_id%2Chidden%2Cmime_extension%2Cmime_type%2Cname%2Cparent_file_id%2Cpunish_flag%2Csize%2Cstarred%2Ctype%2Cupdated_at%2Cdescription)'
    const page = await requestJson(url, { method: 'POST', headers: { 'Content-Type': 'application/json', 'x-share-token': shareToken }, body: JSON.stringify({ share_id: id, parent_file_id: 'root', limit: 100, url_expire_sec: 14400, fields: 'thumbnail', order_by: 'name', order_direction: 'DESC', ...(marker ? { marker } : {}) }) }, '获取阿里云盘分享文件失败')
    items.push(...(Array.isArray(page?.items) ? page.items : []))
    marker = String(page?.next_marker || '')
  } while (marker)
  return items
}

async function importQuarkShare(run: MediaAcquisitionRunView, locator: string, password: string, parentId: string): Promise<ProviderShareImportResult> {
  const id = shareId(locator, 'quark')
  const account = await readProviderToken('quark', run.target.targetUserId)
  const tokenReply = await quarkRequest(account.accessToken, 'share/sharepage/token', { method: 'POST', headers: { referer: `https://pan.quark.cn/s/${id}` }, body: JSON.stringify({ pwd_id: id, passcode: sharePassword(locator, password, 'quark'), support_visit_limit_private_share: true }) }, {}, 'https://drive.quark.cn/1/clouddrive')
  const stoken = String(tokenReply?.data?.stoken || '')
  if (!stoken) throw new Error(tokenReply?.message || '获取夸克分享凭证失败')
  const files = await listQuarkShareFiles(account.accessToken, id, stoken)
  if (!files.length) throw new Error('分享中没有可导入的文件')
  let asyncSubmitted = false
  for (const group of chunks(files, 100)) {
    const reply = await quarkRequest(account.accessToken, 'share/sharepage/save', { method: 'POST', body: JSON.stringify({ fid_list: group.map(item => item.id), fid_token_list: group.map(item => item.token), to_pdir_fid: parentId === 'quark_root' ? '0' : parentId, pwd_id: id, stoken, pdir_fid: '0', pdir_save_all: false, exclude_fids: [], scene: 'link' }) }, {}, 'https://drive.quark.cn/1/clouddrive')
    const task = reply?.data || {}
    if (task?.task_resp?.message && Number(task?.task_resp?.status || 0) >= 400) throw new Error(task.task_resp.message)
    asyncSubmitted ||= !!task.task_id && !task.task_sync
  }
  return { status: asyncSubmitted ? 'async' : 'success', fileCount: files.length, activity: asyncSubmitted ? '已在主进程提交夸克异步分享转存，等待网盘完成' : '已在主进程提交夸克分享转存，正在核对入库目录' }
}

async function listQuarkShareFiles(cookie: string, id: string, stoken: string): Promise<Array<{ id: string; token: string }>> {
  const files: Array<{ id: string; token: string }> = []
  let page = 1
  let hasMore = false
  do {
    const reply = await quarkRequest(cookie, 'share/sharepage/detail', undefined, { pwd_id: id, stoken, pdir_fid: '0', force: '0', _page: page, _size: 100, _fetch_share: 1, _fetch_total: 1, _sort: 'file_type:asc,file_name:asc' }, 'https://drive.quark.cn/1/clouddrive')
    const list = Array.isArray(reply?.data?.list) ? reply.data.list : []
    files.push(...list.map((item: any) => ({ id: String(item?.fid || ''), token: String(item?.share_fid_token || '') })).filter(item => item.id && item.token))
    const total = Number(reply?.data?.metadata?._total || reply?.data?.total || reply?.data?.count || 0)
    hasMore = total ? page * 100 < total : list.length === 100
    page += 1
  } while (hasMore)
  return files
}

async function quarkRequest(cookie: string, path: string, init: RequestInit | undefined, params: Record<string, string | number | boolean>, base: string): Promise<any> {
  const query = new URLSearchParams({ pr: 'ucpro', fr: 'pc', uc_param_str: '', __t: String(Date.now()), __dt: '1000' })
  for (const [key, value] of Object.entries(params)) query.set(key, String(value))
  const response = await fetch(`${base}/${path}?${query}`, { ...init, headers: { accept: 'application/json, text/plain, */*', 'accept-language': 'zh-CN,zh;q=0.9', 'content-type': 'application/json', origin: 'https://pan.quark.cn', referer: 'https://pan.quark.cn/', 'user-agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36', cookie, ...(init?.headers || {}) } })
  const data = await response.json().catch(() => undefined)
  if (!response.ok || data?.status === 'error' || (data?.code !== undefined && data.code !== 0 && data.code !== 200)) throw new Error(data?.message || '夸克分享请求失败')
  return data
}

async function importGuangyaShare(run: MediaAcquisitionRunView, locator: string, password: string, parentId: string): Promise<ProviderShareImportResult> {
  const id = shareId(locator, 'guangya')
  const access = await guangyaPublicPost('/nd.bizuserres.s/v1/get_share_access_token', { shareId: id, code: sharePassword(locator, password, 'guangya') })
  const shareToken = String((access?.data || access)?.accessToken || (access?.data || access)?.access_token || '')
  if (!shareToken) throw new Error('获取光鸭云盘分享凭证失败')
  const files = await listGuangyaShareFiles(shareToken)
  if (!files.length) throw new Error('分享中没有可导入的文件')
  const target = await readProviderToken('guangya', run.target.targetUserId)
  for (const fileIds of chunks(files.map((item: any) => String(item?.fileId || item?.file_id || item?.id || item?.resId || '')).filter(Boolean), 100)) {
    await guangyaAuthenticatedPost(target, '/nd.bizuserres.s/v1/restore_share', { accessToken: shareToken, fileIds, parentId: parentId === 'guangya_root' || parentId === 'guangya' ? '' : parentId })
  }
  return { status: 'success', fileCount: files.length, activity: '已在主进程提交光鸭云盘分享转存，正在核对入库目录' }
}

async function importPikPakShare(run: MediaAcquisitionRunView, locator: string, password: string): Promise<ProviderShareImportResult> {
  const id = shareId(locator, 'pikpak')
  const passCode = sharePassword(locator, password, 'pikpak')
  const query = new URLSearchParams({ limit: '100', thumbnail_size: 'SIZE_LARGE', order: '3', share_id: id, parent_id: '', pass_code: passCode })
  const initial = await requestJson(`${PIKPAK_API_HOST}/drive/v1/share?${query}`, { headers: pikpakPublicHeaders() }, '获取 PikPak 分享凭证失败')
  const shareToken = String(initial?.pass_code_token || initial?.passcode_token || '')
  if (!shareToken) throw new Error('获取 PikPak 分享凭证失败')
  const files = await listPikPakShareFiles(id, shareToken)
  if (!files.length) throw new Error('分享中没有可导入的文件')
  const target = await readProviderToken('pikpak', run.target.targetUserId)
  for (const fileIds of chunks(files.map((item: any) => String(item?.id || '')).filter(Boolean), 100)) {
    await requestJson(`${PIKPAK_API_HOST}/drive/v1/share/restore`, { method: 'POST', headers: pikpakHeaders(target), body: JSON.stringify({ share_id: id, pass_code_token: shareToken, file_ids: fileIds }) }, '保存 PikPak 分享失败')
  }
  return { status: 'success', fileCount: files.length, activity: '已在主进程提交 PikPak 分享转存，正在核对入库目录' }
}

async function listGuangyaShareFiles(accessToken: string): Promise<any[]> {
  const files: any[] = []
  let page = 1
  let hasMore = false
  do {
    const listed = await guangyaPublicPost('/nd.bizuserres.s/v1/get_share_page_files_list', { accessToken, parentId: '', page, pageSize: 100, orderBy: 0, sortType: 0 })
    const body = listed?.data || listed || {}
    const values = body?.list || body?.items || body?.records || body?.content || []
    const items = Array.isArray(values) ? values : []
    files.push(...items)
    const total = Number(body?.total || body?.totalCount || 0)
    hasMore = total ? files.length < total : items.length === 100
    page += 1
  } while (hasMore)
  return files
}

async function listPikPakShareFiles(id: string, shareToken: string): Promise<any[]> {
  const files: any[] = []
  const seen = new Set<string>()
  let pageToken = ''
  do {
    const query = new URLSearchParams({ limit: '100', thumbnail_size: 'SIZE_LARGE', order: '6', share_id: id, parent_id: '', pass_code_token: shareToken, page_token: pageToken })
    const page = await requestJson(`${PIKPAK_API_HOST}/drive/v1/share/detail?${query}`, { headers: pikpakPublicHeaders() }, '获取 PikPak 分享文件失败')
    const values = page?.files || page?.file_list || []
    if (Array.isArray(values)) files.push(...values)
    const next = String(page?.next_page_token || page?.nextPageToken || '')
    if (next && seen.has(next)) throw new Error('PikPak 分享列表分页游标重复')
    if (next) seen.add(next)
    pageToken = next
  } while (pageToken)
  return files
}

function pikpakPublicHeaders(): Record<string, string> { return { 'Content-Type': 'application/json; charset=utf-8', 'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36' } }
function pikpakHeaders(token: { accessToken: string; deviceId: string }): Record<string, string> { return { ...pikpakPublicHeaders(), Authorization: `Bearer ${token.accessToken}`, ...(token.deviceId ? { 'X-Device-Id': token.deviceId } : {}) } }
async function guangyaPublicPost(path: string, body: unknown): Promise<any> { return requestJson(`${GUANGYA_API_URL}${path}`, { method: 'POST', headers: guangyaHeaders({ accessToken: '', deviceId: '' }), body: JSON.stringify(body) }, '光鸭云盘分享请求失败') }
async function guangyaAuthenticatedPost(token: { accessToken: string; deviceId: string }, path: string, body: unknown): Promise<any> { return requestJson(`${GUANGYA_API_URL}${path}`, { method: 'POST', headers: guangyaHeaders(token), body: JSON.stringify(body) }, '光鸭云盘分享转存失败') }
function chunks<T>(items: T[], size: number): T[][] { const output: T[][] = []; for (let index = 0; index < items.length; index += size) output.push(items.slice(index, index + size)); return output }
