# BoxPlayer v4.2.0 Release Notes

> 本次更新带来 **6 大模块、45+ 项重磅升级**,覆盖 AI 阅读 / 智能朗读 / AI 翻译 / 多模型对话 / 专业音乐播放 / 全新下载引擎 / 新增 3 大网盘 / AI 媒体整理代理。

---

## ✨ 新功能

### 📚 全新「图书库」— AI 加持的个人电子书阅读器

#### 🤖 三大 AI 阅读能力(核心亮点)

- **🔊 AI 智能语音朗读 (TTS)**:内置 **Azure 神经语音合成** + Web Speech API 双引擎,支持 **小娴 / 晓晓 / 云希 / 云扬** 等数十种自然中英文音色,可从光标位置 **跨章节连续朗读全文**,自由调节语速 / 音色 / 音调,把任何电子书秒变专业有声书
- **🤖 AI 阅读助手**:一键接入 **OpenAI / DeepSeek / 智谱 GLM / 通义千问 / Moonshot Kimi / 硅基流动 / Ollama 本地大模型 / OpenRouter / Vercel AI Gateway** 共 10+ 主流大模型,支持 **总结本章 / 回答疑问 / 推荐相似书 / 多轮对话**;自带 **本地章节向量 RAG 索引**,深度理解全书内容
- **🌍 AI 划词翻译 + 整书翻译**:选中即译,支持 **AI 翻译(DeepL 级品质)/ Azure / Google 翻译**,可开启 **双语对照阅读** 或 **整书翻译模式**,外语原版书无障碍

#### 📖 全格式阅读器与排版

- **多格式电子书阅读器**:内置全格式电子书引擎,支持 **EPUB / PDF / TXT / MOBI / AZW / AZW3 / FB2 / DOCX / MD / HTML / CBZ / CBR / CB7 / CBT** 全部主流格式
- **三种翻页模式**:单页 / 双页 / 滚动模式自由切换,原生分页与容器滚动稳定生效
- **网盘 + 本地双书源**:所有已接入的网盘自动识别书籍,本地文件夹一键导入,自动刮削封面 / 作者 / 出版日期 / 简介
- **专业排版引擎**:内置 **赫蹏 / 漢字標準格式 / 中文网页重设 typo / Tufte CSS / Typebase** 等多套学术级排版样式,中英文混排比 Kindle 更精致

#### 📒 阅读管理

- **书架 / 收藏 / 标签 / 回收站**:完整生命周期管理,卡片 / 列表 / 封面三种视图自由切换
- **笔记 / 高亮 / 书签 / 划词菜单**:自定义高亮色、附笔记、文中跳转、快捷键操作
- **批量注释导出**:Markdown / JSON / CSV / TXT 一键导出全部高亮与笔记
- **阅读统计**:每日阅读时长 / 翻页数 / 完成度可视化页面
- **OPDS 在线书库订阅**:兼容 OPDS 协议的开放书库即添即用
- **PDF 全文检索 + 章节跳转 + 词典查询 + 文献检索**

---

### 🎵 全新音乐高级播放

- **AudioContext 音效引擎**:10 段 EQ + 混响 + 声像 + 变调不变速 + 实时频谱可视化
- **逐字卡拉 OK 歌词**:基于 Web Animations API 的逐字高亮动画,支持翻译 / 罗马音双行
- **桌面浮动歌词窗口**:独立透明置顶窗口,可拖动定位,随播放进度滚动
- **多源歌词 / 封面 fallback**:LRCLIB 查不到时自动从 网易云 / 酷狗 / QQ音乐 / 酷我 / 咪咕 的开放接口补全歌词与封面(仅查询元数据,不涉及音频流下载)
- **可调主题系统**:12 色可视化编辑器 + 内置 15 套预设主题,全局 CSS 变量化

---

### 📥 全新高速下载引擎

#### 🔧 引擎托管

- **主进程 aria2c 引擎托管**:PID 文件 + 会话续传 + 优雅退出,崩溃自动重连
- **实时事件驱动**:基于 aria2-lib 的低延迟事件订阅 `onDownloadStart/Complete/Error/Stop/BtComplete`,任务状态 100ms 内反馈
- **UPnP 自动端口映射**:BT 下载自动开放 NAT 端口,提升做种连通率
- **BT Tracker 12h 自动同步**:启动后每 12 小时从 [ngosang/trackerslist](https://github.com/ngosang/trackerslist) 拉取最新公共 tracker
- **平台差异化 aria2 配置**:darwin / linux / win32 × x64 / arm64 / armv7l / ia32 共 7 套优化 `aria2.conf`,根据平台和架构自动选择

#### 🖥️ 渲染端体验

- **Torrent 文件选择器**:BT 任务可只下载选中文件,避免拉满整个种子
- **任务详情抽屉**:GID / 总大小 / 进度 / 速度 / 做种数 / 连接数 / InfoHash / 保存路径 / 文件列表 一目了然
- **拖拽添加任务**:从浏览器地址栏 / Finder / 资源管理器拖入 URL / 磁链 / .torrent 文件即建任务
- **协议关联**:`magnet://` / `mo://` 自动捕获并打开下载对话框(支持 macOS `open-url` / `open-file`)
- **下载进度条**:macOS Dock / Windows 任务栏实时显示下载进度环
- **完成系统通知**:每个文件完成弹出系统级通知,点击激活主窗口
- **批量暂停 / 恢复 / 删除**:直连 RPC,无轮询延迟
- **设置项扩充**:上传限速、做种比例、做种时长、自动恢复未完成任务、浏览器扩展 RPC 地址展示、Tracker 编辑框(每行一个 URL)+ 立即同步
- **防休眠管理**:下载进行中阻止系统进入睡眠

---

### 🌐 新增 3 大网盘接入

| 网盘 | App 端能力 | clouddrive-cli `--provider` |
|---|---|---|
| **夸克网盘** | 登录 / 浏览 / 下载 / 上传 / 重命名 / 移动 / 分享 / 搜索 | `quark` |
| **中国移动云盘(139)** | 登录 / 浏览 / 下载 / 上传 / 重命名 / 移动 | `cloud139` |
| **中国电信天翼云盘(189)** | 登录 / 浏览 / 下载 / 上传 / 重命名 / 移动 | `cloud189` |

- clouddrive-cli 同步新增 `cloud139` / `cloud189` / `quark` 三大 provider,并抽出公用的 `ossUpload` / `uploadUtils` 工具,统一断点续传、分片上传、进度回调
- clouddrive-cli 新增 `commandManifest` 与 `mcpToolSchema` 元数据:让 Claude Desktop / Cursor 等 MCP 客户端能自动发现命令、参数与示例

---

### 🤖 AI 媒体整理代理

- **AgentMediaOrganizer 抽屉**:在网盘任意目录右键打开"AI 整理",让 AI 直接看清当前目录、按你描述的规则在网盘中执行重命名 / 移动 / 分类,支持多轮对话
- **基于 Vercel AI SDK**:兼容所有 OpenAI 协议模型(GPT / 通过 Gateway 接入 Claude / DeepSeek / 通义千问 / 智谱 / Moonshot / Ollama 等)
- **可撤销操作日志**:每次 AI 操作都写入 `operationHistory`,可一键回滚到操作前的状态
- **Pan 上下文工具集**:内置 `walkDirectory` / `renamePlan` / `movePlan` / `mediaClassifier` 等可被 AI 调用的安全工具
- **clouddrive-cli `organize` 命令**:把同一套能力暴露给 Claude Code、Cursor 等 AI 终端,远程让 AI 整理云盘

---

### ⚙️ 设置与基础设施重构

- **统一 AI / API 密钥配置页**(`SettingAPI.vue`):集中管理 OpenAI / DeepSeek / Azure TTS / Vercel AI Gateway / 翻译 API 等密钥,所有阅读器、整理代理共享
- **高级下载设置区**(`SettingDownloadAdvanced.vue`):聚合 aria2、做种、Tracker、协议关联等高级参数
- **`shared/` 共享层**:主进程 / 渲染端 / CLI 三方复用的常量、UA、`configKeys`、`tracker`、`rename` 工具函数
- **协议处理重构**:统一的 `electron/main/core/protocol.ts` 处理 magnet / 文件 / 自定义协议,单元测试覆盖
- **aria2 引擎策略**(`aria2EnginePolicy.ts`):根据平台和架构自动选择最佳 aria2c 二进制和配置
- **下载应用框架**:Logger / Context / ConfigManager / Engine / EngineClient / UPnPManager / EnergyManager / ProtocolManager 全套基础设施模块化

---

## 📦 新增主要文件

### 图书库

- `src/layout/book-manager/` · `book-reader/` · `BookReaderModal.vue` · `PageBookLibrary.vue` · `PageBookReader.vue` · `BookCardItem.vue` · `BookCoverItem.vue` · `BookListItem.vue`
- `src/utils/bookAI.ts` · `bookAzureTTS.ts` · `bookTextToSpeech.ts` · `bookReader.ts` · `bookReaderLayout.ts` · `bookReaderState.ts` · `bookNotes.ts` · `bookBookmarks.ts` · `bookOpds.ts` · `bookAnnotationExport.ts` · `bookLookup.ts` · `bookRefer.ts` · `bookScanner.ts` · `bookEpubMeta.ts`
- `src/utils/translators/` · `readerI18n.ts`
- `src/store/booklibrary.ts` · `src/types/{book,bookBookmark,bookNote,bookShelf}.ts`

### 音乐

- `src/module/audioplayer/` · `lyricplayer/` · `musicsdk/` · `theme/`
- `src/lyric/` · `public/mainLyric.html` · `public/pitch-shifter.worklet.js`
- `src/store/musicplayerstore.ts` · `src/components/{SoundEffectBtn,Speedometer,ThemeSelector}.vue`

### 下载引擎

- `electron/main/aria/` — Logger / Context / ConfigManager / Engine / EngineClient / UPnPManager / EnergyManager / ProtocolManager
- `electron/main/core/protocol.ts` + `__tests__/protocol.test.ts`
- `src/down/{TaskDetailDrawer,TorrentFileSelector,UrlDownloadModal}.vue` · `src/components/DragDropZone.vue`
- `src/utils/aria2EnginePolicy.ts`
- `static/engine/{darwin,linux,win32}/*/aria2.conf`(7 份)

### AI 媒体整理

- `src/agent/mediaOrganizer/` · `src/components/AgentMediaOrganizerDrawer.vue` · `src/store/agentMediaOrganizer.ts`
- `clouddrive-cli/core/{commandManifest,mcpToolSchema}.mjs`

### 新增网盘

- `src/quark/` · `src/cloud189/` · `src/cloud139/`
- `clouddrive-cli/providers/{quark,quarkProvider,cloud139,cloud139Provider,cloud189,cloud189Provider,ossUpload,uploadUtils}.mjs`

### 设置与共享层

- `src/setting/{SettingAPI,SettingDownloadAdvanced}.vue`
- `shared/`(常量、UA、configKeys、tracker、rename 工具)

---

## 🔗 主要新增依赖

- `ai`(Vercel AI SDK,AI 阅读助手 + 媒体整理代理底座)
- `microsoft-cognitiveservices-speech-sdk`(Azure 神经 TTS)
- `aria2-lib`(aria2 RPC 客户端)
- `electron-store`(配置持久化)
- `bittorrent-peerid`(Peer ID 解析)

---

**Full Changelog**: `v4.1.0...v4.2.0`
