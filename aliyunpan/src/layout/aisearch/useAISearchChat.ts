import { ref, nextTick } from 'vue'
import { streamText, stepCountIs } from 'ai'
import { createOpenAI } from '@ai-sdk/openai'
import { createOpenAICompatible } from '@ai-sdk/openai-compatible'
import { z } from 'zod'
import Config from '../../config'
import type { AIModelConfig } from '../../utils/bookAI'
import { getAIConfig } from '../../utils/bookAI'
import { isBoxPlayerCloudProvider } from '../../utils/boxplayerCloudAI'
import { getBoxPlayerAccessToken } from '../../utils/boxplayerAuth'
import { searchAllDrives } from '../../utils/globalSearch'
import type { GlobalSearchResult } from '../../utils/globalSearch'
import type { ChatMessage, MessagePart, FileResult, LinkResult } from './types'
import AliShare from '../../aliapi/share'
import AliFileCmd from '../../aliapi/filecmd'
import UserDAL from '../../user/userdal'
import { parseQuarkShareLink } from '../../quark/share'
import { TMDB_BASE_URL, TMDB_BASE_URL_PROXY, tmdbImageUrl } from '../../utils/tmdb'

function getCloudAIBaseURL(): string {
  return ((Config as any).BOXPLAYER_AI_API_URL || 'https://ai.xbyvideohub.com').replace(/\/+$/, '')
}

function createBoxPlayerCloudModel(modelId: string) {
  const baseURL = getCloudAIBaseURL()
  const provider = createOpenAICompatible({
    name: 'boxplayer-cloud',
    baseURL: `${baseURL}/v1`,
    fetch: async (url: any, init?: any) => {
      const token = await getBoxPlayerAccessToken()
      let body = init?.body
      if (typeof body === 'string') {
        try { body = JSON.stringify({ ...JSON.parse(body), feature: 'ai_search' }) } catch {}
      }
      return globalThis.fetch(url, {
        ...init,
        headers: { ...(init?.headers || {}), Authorization: `Bearer ${token}` },
        body,
      })
    },
  })
  return provider(modelId)
}

const CHAT_KEY = 'ai_search_chat_history_v2'

function loadHistory(): ChatMessage[] {
  try {
    const raw = localStorage.getItem(CHAT_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed.filter((m: any) => m?.id && Array.isArray(m.parts))
  } catch {
    return []
  }
}

function saveHistory(messages: ChatMessage[]) {
  try { localStorage.setItem(CHAT_KEY, JSON.stringify(messages.slice(-50))) } catch {}
}

export function useAISearchChat(phSearchFn: (kw: string) => Promise<any>) {
  const messages = ref<ChatMessage[]>(loadHistory())
  const loading = ref(false)
  let abortController: AbortController | null = null

  function appendPart(msgId: string, part: MessagePart) {
    const msg = messages.value.find(m => m.id === msgId)
    if (msg) msg.parts = [...msg.parts, part]
  }

  function getMatchKey(input: any): string | null {
    if (input?.keyword) return `k:${input.keyword}`
    if (input?.url) return `u:${input.url}`
    return null
  }

  function updateToolPart(msgId: string, toolType: string, input: any, fn: (part: any) => void) {
    const msg = messages.value.find(m => m.id === msgId)
    if (!msg) return
    const parts = [...msg.parts]
    const inputKey = getMatchKey(input)
    let idx = -1
    for (let i = parts.length - 1; i >= 0; i--) {
      const p = parts[i]
      if (p.type !== toolType) continue
      if (inputKey) {
        if (getMatchKey((p as any).input) === inputKey) { idx = i; break }
      } else {
        idx = i; break
      }
    }
    if (idx >= 0) {
      const updated = { ...parts[idx] }
      fn(updated)
      parts[idx] = updated
      msg.parts = parts
    }
  }

  function scrollBottom() {
    nextTick(() => {
      const el = document.querySelector('.ai-messages')
      if (el) el.scrollTop = el.scrollHeight
    })
  }

  // scan drives by searching common patterns, never touches media servers
  async function scanAllDrives(platforms?: string[]): Promise<GlobalSearchResult[]> {
    const patterns = ['pdf', 'mp4', 'mkv', 'txt', 'jpg', 'png', 'mp3', 'zip', 'doc', 'xls', 'ppt', 'epub', 'mobi', 'flac', 'rar', '7z', 'avi', 'wmv', 'mov']
    const seen = new Set<string>()
    const results = await Promise.all(
      patterns.map(p =>
        searchAllDrives(p, { platforms, includeMediaServers: false }).catch(() => [] as GlobalSearchResult[])
      )
    )
    const all: GlobalSearchResult[] = []
    for (const batch of results) {
      for (const r of batch) {
        const key = `${r.drive_id}:${r.file_id}`
        if (!seen.has(key)) { seen.add(key); all.push(r) }
      }
    }
    return all
  }

  async function getUserIdForPlatform(platform: 'aliyun' | 'quark'): Promise<{ userId: string; driveId: string } | null> {
    const users = await UserDAL.GetUserListFromDB()
    for (const u of users) {
      if (!u?.user_id || !u?.access_token) continue
      if (platform === 'quark' && u.tokenfrom === 'quark') return { userId: u.user_id, driveId: 'quark' }
      if (platform === 'aliyun' && u.tokenfrom === 'aliyun') return { userId: u.user_id, driveId: u.default_drive_id || '' }
    }
    return null
  }

  async function sendMessage(text: string) {
    const kw = text.trim()
    if (!kw || loading.value) return

    const config = getAIConfig()
    if (!config) {
      const { default: msg } = await import('../../utils/message')
      msg.warning('请先在设置中配置 AI 模型')
      return
    }
    const { default: msg } = await import('../../utils/message')

    const { checkAndIncrement } = await import('../../utils/usageLimit')
    const isBYOK = !isBoxPlayerCloudProvider(config.providerName)
    const uc = checkAndIncrement('aiAgentChat', 1, { metered: false, isBYOK })
    if (!uc.allowed) { msg.warning(uc.message!); return }

    if (abortController) { abortController.abort() }
    abortController = new AbortController()

    const userMsgId = `${Date.now()}-u`
    const aiMsgId = `${Date.now()}-a`

    messages.value = [...messages.value, { id: userMsgId, role: 'user', parts: [{ type: 'text', text: kw }] }]
    messages.value = [...messages.value, { id: aiMsgId, role: 'assistant', parts: [] }]
    loading.value = true
    saveHistory(messages.value)
    scrollBottom()

    try {
      const isOpenAI = config.providerName === 'openai' || config.providerName === 'ai-gateway'
      const model = isBoxPlayerCloudProvider(config.providerName)
        ? createBoxPlayerCloudModel(config.modelId)
        : isOpenAI
          ? createOpenAI({ name: config.providerName || 'openai', apiKey: config.apiKey, baseURL: config.endpoint })(config.modelId)
          : createOpenAICompatible({ name: config.providerName, apiKey: config.apiKey, baseURL: config.endpoint })(config.modelId)

      const apiMessages = messages.value
        .filter(m => m.role === 'user' || m.role === 'assistant')
        .map(m => ({
          role: m.role as 'user' | 'assistant',
          content: m.parts.filter(p => p.type === 'text').map(p => (p as any).text).join('\n'),
        }))

      const result = streamText({
        model,
        system: `你是 BoxPlayer 智能搜索助手。重要：你无法直接访问用户文件，唯一获取文件信息的方式是调用工具。任何涉及文件、网盘、存储的操作都必须调用对应工具。禁止用文字模拟工具结果、禁止编造文件名和大小。查不到就如实说查不到。

## 你的工具
- listDrives: 列出用户所有已登录的网盘
- searchMyFiles: 搜索用户所有云盘中的文件
- searchPanHub: 搜索全网公开网盘分享链接
- findDuplicates: 扫描云盘查找重复文件（需传 platforms 参数限定网盘）
- analyzeStorage: 分析存储空间（需传 platforms 参数限定网盘）
- categorizeFiles: 按类型分类文件（需传 platforms 参数限定网盘）
- importShare: 导入阿里云盘/夸克分享链接，转存到用户网盘
- downloadFiles: 添加文件下载任务
- moveFiles: 移动文件到指定目录（需用户确认）
- getTMDBMovies: 获取 TMDB 最新电影（热映/流行/高分）
- searchTMDB: 在 TMDB 中搜索电影和电视剧，查询影视详细信息
- getDoubanMovies: 获取豆瓣电影排行榜（Top250/新片/口碑/北美票房）
- deleteFiles: 删除文件移入回收站（需用户确认）

## 核心规则（必须遵守）
1. 用户提到文件相关操作，必须调用对应工具，不能只回复文字
2. 禁止在未调用工具的情况下编造文件名、大小等信息
3. 多网盘场景必须先调用 listDrives：
   - 用户要求整理/分析/查重时，先调用 listDrives
   - 用户要求清理重复文件时，提取消息中的 JSON 数组，直接调用 deleteFiles({ files: [...] })，先展示确认信息
   - listDrives 会在界面弹出网盘选择器，让用户勾选并点确定
   - 用户选择后你会收到类似"用户选择了: 阿里云盘(zxm)、百度网盘。platforms: aliyun,baidu"的消息，提取 platforms 列表传给工具
   - 然后你再执行对应操作
   - 用户想看热门电影/新片/排行榜 → 调用 getTMDBMovies（国外）或 getDoubanMovies（国内），无需 listDrives
   - 用户查询特定电影/电视剧信息 → 调用 searchTMDB
   - 导入分享：必须问用户保存到阿里云盘还是夸克
4. 工具返回结果后，简要总结即可
5. moveFiles 和 deleteFiles 必须先展示确认信息
6. 完全无关的问题可以正常简短回复
7. 最多调用工具 5 次

## 回复格式
每次回复必须先写一行【思考】说明你的分析和决策理由，然后再执行操作或给出结论。
例如：
【思考】用户想整理文件。需要先调用 listDrives 列出所有网盘让用户选择。
然后调用对应工具。`,
        messages: apiMessages,
        tools: {
          listDrives: {
            description: '列出用户所有已登录的网盘，让用户在界面中选择要操作的网盘',
            inputSchema: z.object({}),
            execute: async () => {
              const users = await UserDAL.GetUserListFromDB()
              const drives: { userId: string; name: string; platform: string; driveId: string }[] = []
              const seenPlatform = new Set<string>()
              for (const u of users) {
                if (!u?.user_id || !u?.access_token) continue
                const name = u.nick_name || u.user_name || u.name || u.user_id
                const platform = u.tokenfrom || 'aliyun'
                if (seenPlatform.has(platform)) continue
                seenPlatform.add(platform)
                if (platform === 'aliyun') {
                  drives.push({ userId: u.user_id, name, platform, driveId: u.resource_drive_id || u.backup_drive_id || u.default_drive_id || '' })
                } else {
                  drives.push({ userId: u.user_id, name, platform, driveId: u.default_drive_id || '' })
                }
              }
              appendPart(aiMsgId, { type: 'tool-listDrives', state: 'select', drives } as MessagePart)
              scrollBottom()
              return { count: drives.length, drives }
            },
          },

          searchMyFiles: {
            description: '搜索用户所有已登录云盘中的文件',
            inputSchema: z.object({
              keyword: z.string().describe('搜索关键词'),
              platforms: z.array(z.string()).optional().describe('限定搜索的网盘平台名列表，如 ["aliyun","baidu"]'),
            }),
            execute: async (args: any) => {
              const keyword = args.keyword
              appendPart(aiMsgId, {
                type: 'tool-searchMyFiles',
                state: 'running',
                input: { keyword },
              } as MessagePart)
              scrollBottom()

              try {
                const r = await searchAllDrives(keyword, { platforms: args.platforms, includeMediaServers: false })
                const files: FileResult[] = r.slice(0, 30).map((f: GlobalSearchResult) => ({
                  name: f.name, ext: f.ext, size: f.size, isDir: f.isDir,
                  provider: f.provider, providerName: f.providerName,
                  driveId: f.drive_id, fileId: f.file_id,
                  parentFileId: f.parent_file_id, userId: f.user_id, source: f.source,
                }))
                updateToolPart(aiMsgId, 'tool-searchMyFiles', { keyword }, (part: any) => {
                  part.state = 'done'
                  part.output = { total: r.length, files }
                })
                scrollBottom()
                return { total: r.length, files }
              } catch (e: any) {
                updateToolPart(aiMsgId, 'tool-searchMyFiles', { keyword }, (part: any) => {
                  part.state = 'error'
                  part.error = e?.message || '搜索失败'
                })
                scrollBottom()
                return { total: 0, files: [], error: e?.message }
              }
            },
          },
          searchPanHub: {
            description: '搜索全网公开网盘分享链接',
            inputSchema: z.object({ keyword: z.string().describe('搜索关键词') }),
            execute: async (args: any) => {
              const keyword = args.keyword
              const panHubUsage = checkAndIncrement('panHubSearch')
              if (!panHubUsage.allowed) {
                appendPart(aiMsgId, {
                  type: 'tool-searchPanHub',
                  state: 'error',
                  input: { keyword },
                  error: panHubUsage.message || '今日全网资源搜索次数已用完',
                } as MessagePart)
                scrollBottom()
                return { total: 0, links: [], error: panHubUsage.message || '今日全网资源搜索次数已用完' }
              }
              appendPart(aiMsgId, {
                type: 'tool-searchPanHub',
                state: 'running',
                input: { keyword },
              } as MessagePart)
              scrollBottom()

              try {
                const resp = await fetch(
                  `https://api.xbyvideohub.com/api/search?kw=${encodeURIComponent(keyword)}&res=merged_by_type&src=all`
                )
                const d = await resp.json()
                if (d?.code === 0 && d?.data?.merged_by_type) {
                  const all: LinkResult[] = []
                  for (const [, items] of Object.entries(d.data.merged_by_type as Record<string, any[]>)) {
                    all.push(...items.map((i: any) => ({
                      type: i.type || '', url: i.url || '', note: i.note || '', password: i.password || '',
                    })))
                  }
                  updateToolPart(aiMsgId, 'tool-searchPanHub', { keyword }, (part: any) => {
                    part.state = 'done'
                    part.output = { total: all.length, links: all }
                  })
                  scrollBottom()
                  return { total: all.length, links: all.slice(0, 30) }
                }
                const errMsg = d?.message || (d?.code !== 0 ? `API 错误 (code: ${d?.code})` : '未找到匹配资源')
                updateToolPart(aiMsgId, 'tool-searchPanHub', { keyword }, (part: any) => {
                  part.state = 'error'
                  part.error = errMsg
                })
                scrollBottom()
                return { total: 0, links: [], error: errMsg }
              } catch (e: any) {
                updateToolPart(aiMsgId, 'tool-searchPanHub', { keyword }, (part: any) => {
                  part.state = 'error'
                  part.error = e?.message || '搜索失败'
                })
                scrollBottom()
                return { total: 0, links: [], error: e?.message }
              }
            },
          },
          importShare: {
            description: '导入分享链接，将阿里云盘或夸克网盘分享的文件转存到用户网盘',
            inputSchema: z.object({
              url: z.string().describe('分享链接 URL'),
              password: z.string().optional().describe('提取码'),
            }),
            execute: async (args: any) => {
              const { url, password } = args
              const isQuark = /pan\.quark\.cn\/s\//.test(url)
              const isAliyun = /(aliyundrive|alipan)\.com\/s\//.test(url)
              if (!isQuark && !isAliyun) {
                appendPart(aiMsgId, { type: 'tool-importShare', state: 'error', input: { url, password: password || '' }, error: '仅支持阿里云盘和夸克网盘的分享链接' } as MessagePart)
                scrollBottom()
                return { error: 'unsupported platform' }
              }
              appendPart(aiMsgId, { type: 'tool-importShare', state: 'parsing', input: { url, password: password || '' } } as MessagePart)
              scrollBottom()
              const platform = isQuark ? 'quark' : 'aliyun'
              const account = await getUserIdForPlatform(platform)
              if (!account) {
                updateToolPart(aiMsgId, 'tool-importShare', { url }, (p: any) => { p.state = 'error'; p.error = `未登录${platform === 'quark' ? '夸克' : '阿里云'}盘` })
                scrollBottom()
                return { error: 'no account' }
              }
              try {
                let shareToken: string; let shareId: string
                if (isQuark) {
                  const parsed = parseQuarkShareLink(url + (password ? ` 提取码:${password}` : ''))
                  if (!parsed.id) throw new Error('解析夸克分享链接失败')
                  shareId = parsed.id.replace('quark:', '')
                  const { apiQuarkShareToken } = await import('../../quark/share')
                  shareToken = await apiQuarkShareToken(shareId, password || '')
                } else {
                  shareId = url.split(/\.com\/s\/([\w]+)/)[1]
                  shareToken = await AliShare.ApiGetShareToken(shareId, password || '')
                  if (!shareToken || shareToken.startsWith('，')) throw new Error('获取分享token失败')
                }
                updateToolPart(aiMsgId, 'tool-importShare', { url }, (p: any) => { p.state = 'listing' })
                scrollBottom()
                const fileResp = isQuark
                  ? await (await import('../../quark/share')).apiQuarkShareFileList(shareId, shareToken, 'root')
                  : await AliShare.ApiShareFileList(shareId, shareToken, 'root')
                const files = fileResp?.items || []
                if (!files.length) {
                  updateToolPart(aiMsgId, 'tool-importShare', { url }, (p: any) => { p.state = 'done'; p.output = { shareName: '', fileCount: 0, savedCount: 0, platform: platform === 'quark' ? '夸克网盘' : '阿里云盘' } })
                  return { savedCount: 0, fileCount: 0 }
                }
                const fileIds = files.map((f: any) => f.file_id)
                updateToolPart(aiMsgId, 'tool-importShare', { url }, (p: any) => { p.state = 'saving' })
                scrollBottom()
                // refresh Quark cookies from Electron session before saving
        if (isQuark) {
          const { readQuarkCookieStringFromElectron, buildQuarkCookieString } = await import('../../quark/auth')
          const freshCookie = await readQuarkCookieStringFromElectron().catch(() => '')
          if (freshCookie) {
            const { default: UserDAL_ } = await import('../../user/userdal')
            const token = UserDAL_.GetUserToken(account.userId) || await UserDAL_.GetUserTokenFromDB(account.userId)
            if (token) {
              token.access_token = freshCookie
              UserDAL_.SaveUserToken(token)
            }
          }
        }

        const result = await AliShare.ApiSaveShareFilesBatch(shareId, shareToken, account.userId, account.driveId, 'quark_root', fileIds)
                updateToolPart(aiMsgId, 'tool-importShare', { url }, (p: any) => {
                  if (result === 'success') {
                    p.state = 'done'
                    p.output = { shareName: (fileResp as any)?.share_name || files[0]?.name || '', fileCount: files.length, savedCount: files.length, platform: platform === 'quark' ? '夸克网盘' : '阿里云盘' }
                  } else if (result === 'async') {
                    p.state = 'done'
                    p.output = { shareName: (fileResp as any)?.share_name || files[0]?.name || '', fileCount: files.length, savedCount: files.length, platform: platform === 'quark' ? '夸克网盘' : '阿里云盘', asyncStatus: true }
                  } else {
                    p.state = 'error'
                    p.error = result || '转存失败'
                  }
                })
                scrollBottom()
                return { savedCount: result === 'success' ? files.length : 0, fileCount: files.length, shareName: (fileResp as any)?.share_name || '' }
              } catch (e: any) {
                updateToolPart(aiMsgId, 'tool-importShare', { url }, (p: any) => { p.state = 'error'; p.error = e?.message || '导入失败' })
                scrollBottom()
                return { error: e?.message }
              }
            },
          },

          downloadFiles: {
            description: '添加文件下载任务',
            inputSchema: z.object({ files: z.array(z.object({ name: z.string(), fileId: z.string(), driveId: z.string(), userId: z.string() })) }),
            execute: async (args: any) => {
              const files = args.files || []
              if (!files.length) return { total: 0, success: 0 }
              appendPart(aiMsgId, { type: 'tool-downloadFiles', state: 'running', input: { files } } as MessagePart)
              scrollBottom()
              try {
                const { default: DownDAL } = await import('../../down/DownDAL')
                const models = files.map((f: any) => ({ user_id: f.userId, drive_id: f.driveId, file_id: f.fileId, file_name: f.name, parent_file_id: 'root', size: 0, ext: '', category: 'other', icon: '', thumbnail: '', description: '', encType: '', password: '' }))
                DownDAL.aAddDownload(models as any, '', false)
                updateToolPart(aiMsgId, 'tool-downloadFiles', {}, (p: any) => { p.state = 'done'; p.output = { total: files.length, success: files.length } })
                scrollBottom()
                return { total: files.length, success: files.length }
              } catch (e: any) {
                updateToolPart(aiMsgId, 'tool-downloadFiles', {}, (p: any) => { p.state = 'error'; p.error = e?.message || '添加下载失败' })
                scrollBottom()
                return { error: e?.message }
              }
            },
          },

          findDuplicates: {
            description: '扫描云盘查找重复文件，platforms 参数指定要扫描的网盘',
            inputSchema: z.object({ platforms: z.array(z.string()).optional().describe('网盘平台名列表，如 ["aliyun","baidu"]') }),
            execute: async (args: any) => {
              appendPart(aiMsgId, { type: 'tool-findDuplicates', state: 'scanning' } as MessagePart)
              scrollBottom()
              try {
                const allFiles = await scanAllDrives(args.platforms)
                const map = new Map<string, FileResult[]>()
                for (const f of allFiles) {
                  if (f.isDir) continue
                  const key = `${f.name}::${f.size}`
                  if (!map.has(key)) map.set(key, [])
                  map.get(key)!.push({ name: f.name, ext: f.ext, size: f.size, isDir: false, provider: f.provider, providerName: f.providerName, driveId: f.drive_id, fileId: f.file_id, parentFileId: f.parent_file_id, userId: f.user_id, source: f.source })
                }
                const groups = Array.from(map.entries()).filter(([, files]) => files.length > 1).map(([key, files]) => ({ name: key.split('::')[0], size: Number(key.split('::')[1]), files })).sort((a, b) => b.size * (b.files.length - 1) - a.size * (a.files.length - 1)).slice(0, 20)
                updateToolPart(aiMsgId, 'tool-findDuplicates', {}, (p: any) => { p.state = 'done'; p.output = { totalFiles: allFiles.length, groups } })
                scrollBottom()
                return { totalFiles: allFiles.length, groupCount: groups.length }
              } catch (e: any) {
                updateToolPart(aiMsgId, 'tool-findDuplicates', {}, (p: any) => { p.state = 'error'; p.error = e?.message || '扫描失败' })
                scrollBottom()
                return { error: e?.message }
              }
            },
          },

          analyzeStorage: {
            description: '分析存储空间使用情况，platforms 参数指定要分析的网盘',
            inputSchema: z.object({ platforms: z.array(z.string()).optional().describe('网盘平台名列表') }),
            execute: async (args: any) => {
              appendPart(aiMsgId, { type: 'tool-analyzeStorage', state: 'scanning' } as MessagePart)
              scrollBottom()
              try {
                const allFiles = await scanAllDrives(args.platforms)
                const driveMap = new Map<string, { totalSize: number; count: number; files: FileResult[] }>()
                for (const f of allFiles) {
                  const key = f.providerName
                  if (!driveMap.has(key)) driveMap.set(key, { totalSize: 0, count: 0, files: [] })
                  const d = driveMap.get(key)!
                  d.totalSize += f.size; d.count++
                  d.files.push({ name: f.name, ext: f.ext, size: f.size, isDir: f.isDir, provider: f.provider, providerName: f.providerName, driveId: f.drive_id, fileId: f.file_id, parentFileId: f.parent_file_id, userId: f.user_id, source: f.source })
                }
                const drives = Array.from(driveMap.entries()).map(([name, d]) => ({ name, totalSize: d.totalSize, fileCount: d.count, topLarge: [...d.files].sort((a, b) => b.size - a.size).slice(0, 10) }))
                const oldestFiles: FileResult[] = []
                updateToolPart(aiMsgId, 'tool-analyzeStorage', {}, (p: any) => { p.state = 'done'; p.output = { drives, oldestFiles, unusedFiles: [] } })
                scrollBottom()
                return { drives: drives.length, totalFiles: allFiles.length }
              } catch (e: any) {
                updateToolPart(aiMsgId, 'tool-analyzeStorage', {}, (p: any) => { p.state = 'error'; p.error = e?.message || '分析失败' })
                scrollBottom()
                return { error: e?.message }
              }
            },
          },

          categorizeFiles: {
            description: '分析文件类型分布，platforms 参数指定要分类的网盘',
            inputSchema: z.object({ platforms: z.array(z.string()).optional().describe('网盘平台名列表') }),
            execute: async (args: any) => {
              appendPart(aiMsgId, { type: 'tool-categorizeFiles', state: 'planning' } as MessagePart)
              scrollBottom()
              try {
                const allFiles = await scanAllDrives(args.platforms)
                const catMap: Record<string, { exts: string[]; count: number; size: number }> = {
                  '视频': { exts: ['mp4','mkv','avi','mov','wmv','flv','webm'], count: 0, size: 0 },
                  '文档': { exts: ['pdf','doc','docx','txt','md','xls','xlsx','ppt','pptx'], count: 0, size: 0 },
                  '音频': { exts: ['mp3','flac','wav','aac','ogg','m4a'], count: 0, size: 0 },
                  '图片': { exts: ['jpg','jpeg','png','gif','bmp','webp','svg'], count: 0, size: 0 },
                  '压缩包': { exts: ['zip','rar','7z','tar','gz'], count: 0, size: 0 },
                  '其他': { exts: [], count: 0, size: 0 },
                }
                for (const f of allFiles) {
                  if (f.isDir) continue
                  const ext = (f.ext || '').toLowerCase()
                  let found = false
                  for (const [name, cat] of Object.entries(catMap)) {
                    if (name === '其他') continue
                    if (cat.exts.includes(ext)) { cat.count++; cat.size += f.size; found = true; break }
                  }
                  if (!found) { catMap['其他'].count++; catMap['其他'].size += f.size }
                }
                const categories = Object.entries(catMap).filter(([, c]) => c.count > 0).map(([name, c]) => ({ name, pattern: c.exts.slice(0, 5).join(', ') + (c.exts.length > 5 ? '…' : ''), fileCount: c.count, totalSize: c.size }))
                updateToolPart(aiMsgId, 'tool-categorizeFiles', {}, (p: any) => { p.state = 'done'; p.output = { categories } })
                scrollBottom()
                return { categoryCount: categories.length }
              } catch (e: any) {
                updateToolPart(aiMsgId, 'tool-categorizeFiles', {}, (p: any) => { p.state = 'error'; p.error = e?.message || '分析失败' })
                scrollBottom()
                return { error: e?.message }
              }
            },
          },

          moveFiles: {
            description: '移动文件到指定目录（需要用户确认）',
            inputSchema: z.object({ files: z.array(z.object({ name: z.string(), fileId: z.string(), driveId: z.string(), userId: z.string() })), targetDir: z.string() }),
            execute: async (args: any) => {
              const { files, targetDir } = args
              if (!files?.length) return { total: 0, success: 0 }
              appendPart(aiMsgId, { type: 'tool-moveFiles', state: 'confirm', input: { files, targetDir } } as MessagePart)
              scrollBottom()
              return { pending: true }
            },
          },

          getTMDBMovies: {
            description: '从 TMDB 获取电影数据（热映、流行、高分），获取最新电影和电视剧信息',
            inputSchema: z.object({
              category: z.enum(['trending','popular','top_rated']).optional().describe('类别：trending=热映, popular=流行, top_rated=高分'),
              page: z.number().optional().describe('页码，默认1'),
            }),
            execute: async (args: any) => {
              const category: string = args.category || 'trending'
              const page = args.page || 1
              const labelMap: Record<string, string> = { trending: 'TMDB 热映', popular: 'TMDB 流行', top_rated: 'TMDB 高分' }
              const label = labelMap[category] || 'TMDB 电影'
              appendPart(aiMsgId, { type: 'tool-getMovies', state: 'loading', category: label } as MessagePart)
              scrollBottom()
              let items: { title: string; desc: string }[] = []
              try {
                let movieResults: any[] = []
                let tvResults: any[] = []

                if (category === 'top_rated') {
                  const resp = await fetch(`${TMDB_BASE_URL}/movie/top_rated?language=zh-CN`)
                  const data = await resp.json()
                  if (data?.code === 0) {
                    movieResults = data.movies || []
                    tvResults = data.tv || []
                  }
                } else {
                  const typeMap: Record<string, string[]> = {
                    trending: ['/trending/movie/week', '/trending/tv/week'],
                    popular: ['/movie/popular', '/tv/popular'],
                  }
                  const endpoints = typeMap[category] || ['/trending/movie/week', '/trending/tv/week']
                  const [movieResp, tvResp] = await Promise.all([
                    fetch(`${TMDB_BASE_URL_PROXY}${endpoints[0]}?language=zh-CN&page=${page}`),
                    fetch(`${TMDB_BASE_URL_PROXY}${endpoints[1]}?language=zh-CN&page=${page}`),
                  ])
                  const movieData = await movieResp.json()
                  const tvData = await tvResp.json()
                  if (movieData?.results) movieResults = movieData.results
                  if (tvData?.results) tvResults = tvData.results
                }

                const movieItems = movieResults.slice(0, 25).map((m: any) => ({
                  id: String(m.id || ''),
                  title: m.title || m.name || '',
                  cover: tmdbImageUrl(m.poster_path),
                  desc: `评分: ${m.vote_average?.toFixed(1) || '?'} · ${(m.release_date || m.first_air_date || '').slice(0, 4)}`,
                  url: `https://www.themoviedb.org/movie/${m.id}`,
                }))
                const tvItems = tvResults.slice(0, 10).map((t: any) => ({
                  id: String(t.id || ''),
                  title: `📺 ${t.name || t.title || ''}`,
                  cover: tmdbImageUrl(t.poster_path),
                  desc: `评分: ${t.vote_average?.toFixed(1) || '?'} · ${(t.first_air_date || '').slice(0, 4)}`,
                  url: `https://www.themoviedb.org/tv/${t.id}`,
                }))
                const allItems = [...movieItems, ...tvItems]
                items = allItems.map(i => ({ title: i.title, desc: i.desc }))

                if (movieItems.length || tvItems.length) {
                  updateToolPart(aiMsgId, 'tool-getMovies', {}, (p: any) => { p.state = 'done'; p.category = `${label} · ${movieItems.length + tvItems.length}部`; p.movies = movieItems; p.tv = tvItems })
                } else {
                  updateToolPart(aiMsgId, 'tool-getMovies', {}, (p: any) => { p.state = 'error'; p.error = '获取失败' })
                }
              } catch (e: any) {
                updateToolPart(aiMsgId, 'tool-getMovies', {}, (p: any) => { p.state = 'error'; p.error = e?.message || '请求失败' })
              }
              scrollBottom()
              return { category: label, total: items.length, items }
            },
          },

          getDoubanMovies: {
            description: '获取豆瓣电影排行榜（Top250、新片榜、口碑榜、北美票房），用户想看热门电影或了解最近什么电影好看时调用',
            inputSchema: z.object({ category: z.enum(['douban-top250','douban-movie','douban-weekly','douban-us-box']).optional().describe('排行榜类别，默认douban-top250') }),
            execute: async (args: any) => {
              const category = args.category || 'douban-top250'
              appendPart(aiMsgId, { type: 'tool-getMovies', state: 'loading', category } as MessagePart)
              scrollBottom()
              let items: { title: string; desc: string }[] = []
              try {
                const resp = await fetch(`https://api.xbyvideohub.com/api/douban-hot?category=${category}&page=1&limit=25`)
                const data = await resp.json()
                if (data?.code === 0 && data?.data?.items) {
                  const movies = data.data.items.map((item: any) => ({ id: String(item.id || ''), title: item.title || '', cover: item.cover || '', desc: item.desc || '', url: item.url || '' }))
                  items = movies.map((m: any) => ({ title: m.title, desc: m.desc }))
                  updateToolPart(aiMsgId, 'tool-getMovies', {}, (p: any) => { p.state = 'done'; p.category = category; p.movies = movies })
                } else {
                  updateToolPart(aiMsgId, 'tool-getMovies', {}, (p: any) => { p.state = 'error'; p.error = data?.message || '获取电影数据失败' })
                }
              } catch (e: any) {
                updateToolPart(aiMsgId, 'tool-getMovies', {}, (p: any) => { p.state = 'error'; p.error = e?.message || '请求失败' })
              }
              scrollBottom()
              return { category, total: items.length, items }
            },
          },

          searchTMDB: {
            description: '在 TMDB 中搜索电影和电视剧，查询详细的影视信息',
            inputSchema: z.object({ query: z.string().describe('搜索关键词') }),
            execute: async (args: any) => {
              const query = args.query
              appendPart(aiMsgId, { type: 'tool-getMovies', state: 'loading', category: `搜索: ${query}` } as MessagePart)
              scrollBottom()
              let items: { title: string; desc: string }[] = []
              try {
                const [movieResp, tvResp] = await Promise.all([
                  fetch(`${TMDB_BASE_URL_PROXY}/search/movie?query=${encodeURIComponent(query)}&language=zh-CN`),
                  fetch(`${TMDB_BASE_URL_PROXY}/search/tv?query=${encodeURIComponent(query)}&language=zh-CN`),
                ])
                const movieData = await movieResp.json()
                const tvData = await tvResp.json()
                const movieResults: any[] = movieData?.results || []
                const tvResults: any[] = tvData?.results || []

                const movies = movieResults.slice(0, 10).map((m: any) => ({
                  id: String(m.id || ''),
                  title: m.title || '',
                  cover: tmdbImageUrl(m.poster_path),
                  desc: `评分: ${m.vote_average?.toFixed(1) || '?'} · ${(m.release_date || '').slice(0, 4)}`,
                  url: `https://www.themoviedb.org/movie/${m.id}`,
                }))
                const tvItems = tvResults.slice(0, 10).map((t: any) => ({
                  id: String(t.id || ''),
                  title: `📺 ${t.name || ''}`,
                  cover: tmdbImageUrl(t.poster_path),
                  desc: `评分: ${t.vote_average?.toFixed(1) || '?'} · ${(t.first_air_date || '').slice(0, 4)}`,
                  url: `https://www.themoviedb.org/tv/${t.id}`,
                }))
                const allResults = [...movies, ...tvItems]
                items = allResults.map((m: any) => ({ title: m.title, desc: m.desc }))
                updateToolPart(aiMsgId, 'tool-getMovies', {}, (p: any) => { p.state = 'done'; p.category = `搜索: ${query} · ${allResults.length}个结果`; p.movies = movies; p.tv = tvItems })
              } catch (e: any) {
                updateToolPart(aiMsgId, 'tool-getMovies', {}, (p: any) => { p.state = 'error'; p.error = e?.message || '请求失败' })
              }
              scrollBottom()
              return { query, total: items.length, items }
            },
          },

          deleteFiles: {
            description: '删除文件（需要用户确认，移入回收站）',
            inputSchema: z.object({ files: z.array(z.object({ name: z.string(), fileId: z.string(), driveId: z.string(), userId: z.string() })) }),
            execute: async (args: any) => {
              const { files } = args
              if (!files?.length) return { total: 0, success: 0 }
              appendPart(aiMsgId, { type: 'tool-deleteFiles', state: 'confirm', input: { files } } as MessagePart)
              scrollBottom()
              return { pending: true }
            },
          },
        },
        toolChoice: 'auto',
          stopWhen: stepCountIs(5),
        temperature: 0.7,
        abortSignal: abortController?.signal,
      })

      // stream text delta, split reasoning from text
      let textPart: any = null
      let reasoningPart: any = null
      for await (const part of result.fullStream) {
        if (part.type === 'text-delta') {
          const delta = part.text
          // detect reasoning block: 【思考】... or 【分析】...
          const reasoningMatch = delta.match(/【(思考|分析)】([\\s\\S]*)/)
          if (reasoningMatch && !reasoningPart) {
            reasoningPart = { type: 'reasoning', text: reasoningMatch[2] }
            appendPart(aiMsgId, reasoningPart)
            const afterReasoning = delta.slice(reasoningMatch[0].length).trim()
            if (afterReasoning) {
              textPart = { type: 'text', text: afterReasoning }
              appendPart(aiMsgId, textPart)
            }
          } else if (reasoningPart && !textPart) {
            reasoningPart.text += delta
          } else {
            if (!textPart) {
              textPart = { type: 'text', text: '' }
              appendPart(aiMsgId, textPart)
            }
            textPart.text += delta
          }
          scrollBottom()
        }
      }
    } catch (e: any) {
      if (e?.name === 'AbortError') return
      appendPart(aiMsgId, {
        type: 'text',
        text: `\n\n❌ ${e?.message || 'AI 请求失败'}`,
      })
    } finally {
      loading.value = false
      saveHistory(messages.value)
    }
  }

  function clear() {
    if (abortController) { abortController.abort(); abortController = null }
    messages.value = []
    loading.value = false
    localStorage.removeItem(CHAT_KEY)
  }

  async function confirmAction(msgId: string, partIndex: number) {
    const msg = messages.value.find(m => m.id === msgId)
    if (!msg) return
    const part = msg.parts[partIndex] as any
    if (!part || (part.type !== 'tool-moveFiles' && part.type !== 'tool-deleteFiles')) return
    if (part.state !== 'confirm') return
    part.state = 'running'
    scrollBottom()
    try {
      const { files, targetDir } = part.input || {}
      const userId = files[0]?.userId
      const driveId = files[0]?.driveId
      const fileIds = files.map((f: any) => f.fileId)
      if (part.type === 'tool-moveFiles') {
        const result = await AliFileCmd.ApiMoveBatch(userId, driveId, fileIds, driveId, targetDir || 'root')
        const failed = result?.length || 0
        part.state = 'done'
        part.output = { total: files.length, success: files.length - failed, failed }
      } else {
        const result = await AliFileCmd.ApiDeleteBatch(userId, driveId, fileIds)
        const failed = result?.length || 0
        part.state = 'done'
        part.output = { total: files.length, success: files.length - failed, failed }
      }
    } catch (e: any) {
      part.state = 'error'
      part.error = e?.message || '操作失败'
    }
    scrollBottom()
    saveHistory(messages.value)
  }

  function cancelAction(msgId: string, partIndex: number) {
    const msg = messages.value.find(m => m.id === msgId)
    if (!msg) return
    const part = msg.parts[partIndex] as any
    if (part && (part.type === 'tool-moveFiles' || part.type === 'tool-deleteFiles') && part.state === 'confirm') {
      part.state = 'done'
      part.output = { total: part.input?.files?.length || 0, success: 0, failed: 0 }
    }
    saveHistory(messages.value)
  }

  return { messages, loading, sendMessage, clear, confirmAction, cancelAction }
}
