export type BoxPlayerCapabilityMode = 'read' | 'write' | 'destructive' | 'navigate'
export type BoxPlayerCapabilityStatus = 'available' | 'guided' | 'planned'

export interface BoxPlayerCapability {
  id: string
  title: string
  module: string
  description: string
  status: BoxPlayerCapabilityStatus
  mode: BoxPlayerCapabilityMode
  tools: string[]
  limitations: string[]
  examples: string[]
  tab?: string
  menu?: string
}

// This registry is the single knowledge source for the workspace Agent. Registering
// a capability describes it to the model, but only a name in `tools` can execute.
export const BOXPLAYER_CAPABILITIES: BoxPlayerCapability[] = [
  {
    id: 'cloud-files', title: '网盘文件', module: '网盘', status: 'available', mode: 'write', tab: 'pan',
    description: '查看已登录网盘，搜索文件，分析空间，扫描重复项/大文件/空目录，并在确认后整理、下载或移入回收站。',
    tools: ['listDrives', 'searchMyFiles', 'analyzeStorage', 'categorizeFiles', 'findDuplicates', 'scanDriveLargeFiles', 'scanDriveEmptyDirs', 'moveFiles', 'organizeFiles', 'mediaOrganizeFiles', 'downloadFiles', 'deleteFiles'],
    limitations: ['跨网盘操作受各平台能力限制', '移动、整理、删除必须由用户确认'],
    examples: ['查找所有网盘里的《三体》', '分析阿里云盘和夸克的空间', '扫描 1GB 以上的视频']
  },
  {
    id: 'sharing', title: '分享与直链', module: '分享', status: 'available', mode: 'write', tab: 'share',
    description: '搜索公开资源、导入阿里云盘或夸克分享，并导出已选文件的下载直链。',
    tools: ['searchPanHub', 'importShare', 'exportDirectLinks'],
    limitations: ['分享导入目前仅支持阿里云盘和夸克', '创建新分享链接尚未开放给 Agent'],
    examples: ['搜索公开的电影资源', '导入这个夸克分享链接', '把这些文件导出为 aria2 链接']
  },
  {
    id: 'transfers', title: '传输与离线任务', module: '传输', status: 'available', mode: 'write', tab: 'down',
    description: '创建文件下载任务，解析秒传清单，并在确认后导入光鸭或提交磁力链接。',
    tools: ['downloadFiles', 'parseMiaochuanJson', 'importMiaochuanToGuangya', 'importGuangyaMagnets'],
    limitations: ['上传、新建离线任务之外的传输设置仍需在界面操作', '导入光鸭和磁力任务必须确认'],
    examples: ['下载搜索到的文件', '解析这段秒传 JSON', '把这些 magnet 导入光鸭']
  },
  {
    id: 'media-discovery', title: '影视发现', module: '视频', status: 'available', mode: 'read', tab: 'media',
    description: '查询 TMDB 和豆瓣的电影、电视剧、榜单和基础详情。',
    tools: ['getTMDBMovies', 'searchTMDB', 'getDoubanMovies'],
    limitations: ['这是影视资料查询，不会替用户自动播放或修改片库'],
    examples: ['推荐近期口碑电影', '搜索《沙丘》电视剧和电影', '看豆瓣新片榜']
  },
  {
    id: 'media-acquisition', title: '媒体获取 Agent', module: '搜索/AI 工作台/视频', status: 'available', mode: 'write', tab: 'ai',
    description: '从搜索结果创建媒体获取任务，按目标网盘能力优先分享导入并 fallback 到磁力/外链离线，查询实时活动、通知和追更缺集状态。',
    tools: ['listMediaAcquisitionTasks', 'listMediaAcquisitionTracking', 'listMediaAcquisitionNotifications', 'runMediaAcquisitionPatrol', 'openBoxPlayerModule'],
    limitations: ['创建获取任务必须由用户在弹窗中选择目标网盘和保存目录', 'Agent 只会使用用户选择的目标网盘，不会自动切换其它账号', '分享导入失败时仅在目标网盘支持离线下载时继续 fallback'],
    examples: ['查看正在获取的媒体任务', '哪些剧还有缺集', '触发一次追更巡检']
  },
  {
    id: 'upload', title: '上传', module: '网盘', status: 'guided', mode: 'navigate', tab: 'pan',
    description: '可打开网盘工作区，让用户选择文件或文件夹上传。',
    tools: ['openBoxPlayerModule'],
    limitations: ['Agent 不读取本机文件系统，也不会代替用户选择本地文件'],
    examples: ['带我去上传文件']
  },
  {
    id: 'playback', title: '播放', module: '视频/音乐', status: 'guided', mode: 'navigate', tab: 'media',
    description: '可打开媒体库；具体播放、清晰度、字幕和音轨选择保留在播放器界面。',
    tools: ['openBoxPlayerModule'],
    limitations: ['Agent 暂不直接控制正在播放的内容'],
    examples: ['打开视频媒体库']
  },
  {
    id: 'media-server', title: '媒体服务器', module: '媒体服务器', status: 'guided', mode: 'navigate', tab: 'media-server',
    description: '可打开 Emby、Jellyfin 或 Plex 的媒体服务器工作区。',
    tools: ['openBoxPlayerModule'],
    limitations: ['登录、播放和服务器配置仍在媒体服务器界面完成'],
    examples: ['打开媒体服务器']
  },
  {
    id: 'books', title: '书籍与阅读', module: '书籍', status: 'guided', mode: 'navigate', tab: 'book',
    description: '可打开书籍库。单本书的问答和检索由 Reedy 阅读助手独立处理。',
    tools: ['openBoxPlayerModule'],
    limitations: ['工作台不会直接读取未选择的书籍内容'],
    examples: ['打开书籍库']
  },
  {
    id: 'settings', title: '应用设置', module: '设置', status: 'guided', mode: 'navigate', tab: 'setting',
    description: '可打开设置中心，供用户配置 AI、播放器、网盘、网络和下载选项。',
    tools: ['openBoxPlayerModule'],
    limitations: ['Agent 暂不能修改设置，避免意外改变隐私、网络和传输配置'],
    examples: ['打开 AI 模型设置', '带我去下载设置']
  }
]

export function getBoxPlayerCapability(id: string): BoxPlayerCapability | undefined {
  return BOXPLAYER_CAPABILITIES.find(capability => capability.id === id)
}

export function buildBoxPlayerCapabilityKnowledge(): string {
  return BOXPLAYER_CAPABILITIES.map(capability => [
    `### ${capability.title}（${capability.status === 'available' ? '可执行' : '引导模式'}）`,
    capability.description,
    `可用工具：${capability.tools.join('、')}`,
    `限制：${capability.limitations.join('；')}`,
    `示例：${capability.examples.join('；')}`
  ].join('\n')).join('\n\n')
}
