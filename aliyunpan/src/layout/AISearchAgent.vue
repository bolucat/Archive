<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { Sparkles, User, Bot, Loader2 } from 'lucide-vue-next'
import { useAppStore } from '../store'
import message from '../utils/message'
import { useAISearchChat } from './aisearch/useAISearchChat'
import ReasonChain from './aisearch/ReasonChain.vue'
import ClarifyCard from './aisearch/ClarifyCard.vue'
import SearchFilesCard from './aisearch/SearchFilesCard.vue'
import SearchLinksCard from './aisearch/SearchLinksCard.vue'
import SummaryCard from './aisearch/SummaryCard.vue'
import LoadingIndicator from './aisearch/LoadingIndicator.vue'
import DriveSelectorCard from './aisearch/DriveSelectorCard.vue'
import ImportShareCard from './aisearch/ImportShareCard.vue'
import DownloadCard from './aisearch/DownloadCard.vue'
import DuplicateCard from './aisearch/DuplicateCard.vue'
import StorageCard from './aisearch/StorageCard.vue'
import OrganizeCard from './aisearch/OrganizeCard.vue'
import BatchActionCard from './aisearch/BatchActionCard.vue'
import MovieListCard from './aisearch/MovieListCard.vue'
import { renderMarkdown } from './aisearch/markdown'
import type { FileResult } from './aisearch/types'

const props = defineProps<{ aiEnabled: boolean; keyword: string; trigger: number; phSearch: (kw: string) => Promise<any> }>()
const emit = defineEmits<{ 'search-resource': [title: string] }>()

const appStore = useAppStore()
const { messages, loading, sendMessage, clear, confirmAction, cancelAction } = useAISearchChat(props.phSearch)
const chatContainer = ref<HTMLElement>()
const inputText = ref('')

function handleSend(q?: string) {
  const kw = (q || props.keyword || '').trim()
  if (!kw) return false
  if (!props.aiEnabled) {
    message.warning('AI Agent 对话需登录后配置 BYOK 模型或升级 Pro 后使用')
    return false
  }
  sendMessage(kw)
  return true
}

function handleInputSend() {
  if (handleSend(inputText.value)) inputText.value = ''
}

function handleSearchResource(title: string) {
  emit('search-resource', title)
}

function handleClarifySelect(option: string) {
  handleSend(option)
}

function handleFollowUp(query: string) {
  handleSend(query)
}

function handleFileNavigate(file: FileResult) {
  appStore.toggleTab('pan')
  nextTick(async () => {
    const { default: usePanTreeStore } = await import('../pan/pantreestore')
    const { default: PanDAL } = await import('../pan/pandal')
    const { default: UserDAL } = await import('../user/userdal')
    const panTreeStore = usePanTreeStore()
    if (panTreeStore.user_id !== file.userId) await UserDAL.UserChange(file.userId)
    await nextTick()
    let fileId = file.parentFileId || file.fileId
    if (fileId === '/' || !fileId) {
      const map: Record<string, string> = {
        baidu: 'baidu_root', cloud123: 'cloud_root', '115': 'drive115_root',
        quark: 'quark_root', pikpak: 'pikpak_root', dropbox: 'dropbox_root',
        onedrive: 'onedrive_root', box: 'box_root',
      }
      fileId = map[file.provider] || fileId
    }
    PanDAL.aReLoadOneDirToShow(file.driveId || fileId, fileId, true)
  })
}

function handleRetryTool(_messageId: string, _toolType: string, input: any) {
  if (input?.keyword) handleSend(input.keyword)
}

function handleDriveConfirm(selected: { userId: string; name: string; platform: string; driveId: string }[]) {
  const PLATFORM_LABELS: Record<string, string> = {
    aliyun: '阿里云盘', quark: '夸克网盘', baidu: '百度网盘', '115': '115网盘',
    '123': '123云盘', tianyi: '天翼云盘', xunlei: '迅雷云盘', pikpak: 'PikPak',
    dropbox: 'Dropbox', onedrive: 'OneDrive', box: 'Box',
  }
  const desc = selected.map(d => `${PLATFORM_LABELS[d.platform] || d.platform}(${d.name})`).join('、')
  const platforms = selected.map(d => d.platform)
  handleSend(`用户选择了: ${desc}。platforms: ${platforms.join(',')}`)
}

function handleConfirmAction(msgId: string, partIndex: number) { confirmAction(msgId, partIndex) }
function handleCancelAction(msgId: string, partIndex: number) { cancelAction(msgId, partIndex) }

watch(() => props.trigger, () => {
  if (props.keyword.trim()) handleSend()
})

const DEFAULT_FOLLOWUPS = [
  '豆瓣电影排行榜',
  'TMDB 最近热映',
  '帮我找科幻电影',
  '最近有什么好电影',
  '整理文件',
  '清理空间',
  '查找重复文件',
  '分析存储空间',
]
</script>

<template>
  <div class="ai-chat">
    <!-- messages -->
    <div ref="chatContainer" class="ai-messages">
      <!-- empty -->
      <div v-if="messages.length === 0 && !loading" class="ai-empty">
        <Sparkles :size="48" :stroke-width="1" class="ai-empty-icon" />
        <div class="ai-empty-title">AI 智能搜索</div>
        <div class="ai-empty-desc">{{ aiEnabled ? '在下方输入框描述你想做什么，AI 帮你搜索和整理文件' : '未登录用户可以预览 AI Agent；登录后配置 BYOK 或升级 Pro 才能发送对话' }}</div>
      </div>

      <!-- message list -->
      <div v-else>
      <div
        v-for="msg in messages"
        :key="msg.id"
        class="ai-msg"
        :class="'ai-msg--' + msg.role"
      >
        <!-- avatar -->
        <div class="ai-msg-avatar">
          <User v-if="msg.role === 'user'" :size="16" :stroke-width="2" />
          <Bot v-else :size="16" :stroke-width="2" />
        </div>

        <!-- parts -->
        <div class="ai-msg-body">
          <template v-for="(part, pi) in msg.parts" :key="pi">
            <!-- tool: listDrives -->
            <DriveSelectorCard
              v-if="part.type === 'tool-listDrives'"
              :drives="(part as any).drives"
              @confirm="handleDriveConfirm"
            />

            <!-- text (user) -->
            <div v-else-if="part.type === 'text' && msg.role === 'user'" class="ai-msg-text">{{ (part as any).text }}</div>

            <!-- text (assistant, markdown) -->
            <div v-else-if="part.type === 'text'" class="ai-msg-text" v-html="renderMarkdown((part as any).text)" />

            <!-- reasoning -->
            <ReasonChain v-else-if="part.type === 'reasoning'" :text="(part as any).text" />

            <!-- clarification -->
            <ClarifyCard
              v-else-if="part.type === 'clarification'"
              :question="(part as any).question"
              :options="(part as any).options"
              @select="handleClarifySelect"
            />

            <!-- tool: searchMyFiles -->
            <SearchFilesCard
              v-else-if="part.type === 'tool-searchMyFiles'"
              :state="(part as any).state"
              :input="(part as any).input"
              :output="(part as any).output"
              :error="(part as any).error"
              @navigate="handleFileNavigate"
              @retry="handleRetryTool(msg.id, 'tool-searchMyFiles', (part as any).input)"
            />

            <!-- tool: searchPanHub -->
            <SearchLinksCard
              v-else-if="part.type === 'tool-searchPanHub'"
              :state="(part as any).state"
              :input="(part as any).input"
              :output="(part as any).output"
              :error="(part as any).error"
              @retry="handleRetryTool(msg.id, 'tool-searchPanHub', (part as any).input)"
            />

            <!-- summary -->
            <SummaryCard
              v-else-if="part.type === 'summary'"
              :text="(part as any).text"
              :followups="(part as any).followups"
              @followup="handleFollowUp"
            />

            <!-- tool: importShare -->
            <ImportShareCard
              v-else-if="part.type === 'tool-importShare'"
              :state="(part as any).state"
              :input="(part as any).input"
              :output="(part as any).output"
              :error="(part as any).error"
              @retry="handleRetryTool(msg.id, 'tool-importShare', (part as any).input)"
            />

            <!-- tool: downloadFiles -->
            <DownloadCard
              v-else-if="part.type === 'tool-downloadFiles'"
              :state="(part as any).state"
              :input="(part as any).input"
              :output="(part as any).output"
              :error="(part as any).error"
              @retry="handleRetryTool(msg.id, 'tool-downloadFiles', (part as any).input)"
            />

            <!-- tool: getMovies -->
            <MovieListCard
              v-else-if="part.type === 'tool-getMovies'"
              :state="(part as any).state"
              :category="(part as any).category"
              :movies="(part as any).movies"
              :tv="(part as any).tv"
              :error="(part as any).error"
              @search="(title: string) => handleSend(title)"
              @search-resource="(title: string) => handleSearchResource(title)"
              @retry="handleRetryTool(msg.id, 'tool-getMovies', {})"
            />

            <!-- tool: findDuplicates -->
            <DuplicateCard
              v-else-if="part.type === 'tool-findDuplicates'"
              :state="(part as any).state"
              :output="(part as any).output"
              :error="(part as any).error"
              @navigate="handleFileNavigate"
              @select="(files: FileResult[]) => handleSend('删除以下' + files.length + '个重复文件: ' + JSON.stringify(files.map(f => ({name:f.name,fileId:f.fileId,driveId:f.driveId,userId:f.userId}))))"
              @retry="handleRetryTool(msg.id, 'tool-findDuplicates', {})"
            />

            <!-- tool: analyzeStorage -->
            <StorageCard
              v-else-if="part.type === 'tool-analyzeStorage'"
              :state="(part as any).state"
              :output="(part as any).output"
              :error="(part as any).error"
              @navigate="handleFileNavigate"
              @retry="handleRetryTool(msg.id, 'tool-analyzeStorage', {})"
            />

            <!-- tool: categorizeFiles -->
            <OrganizeCard
              v-else-if="part.type === 'tool-categorizeFiles'"
              :state="(part as any).state"
              :output="(part as any).output"
              :error="(part as any).error"
              @organize="(cats: any[]) => handleSend('帮我把文件按以下分类整理: ' + cats.map((c: any) => c.name + '→' + c.name + '文件夹').join(', '))"
              @retry="handleRetryTool(msg.id, 'tool-categorizeFiles', {})"
            />

            <!-- tool: moveFiles -->
            <BatchActionCard
              v-else-if="part.type === 'tool-moveFiles'"
              action="move"
              :state="(part as any).state"
              :files="(part as any).input?.files || []"
              :target-dir="(part as any).input?.targetDir"
              :output="(part as any).output"
              :error="(part as any).error"
              @confirm="handleConfirmAction(msg.id, pi)"
              @cancel="handleCancelAction(msg.id, pi)"
            />

            <!-- tool: deleteFiles -->
            <BatchActionCard
              v-else-if="part.type === 'tool-deleteFiles'"
              action="delete"
              :state="(part as any).state"
              :files="(part as any).input?.files || []"
              :output="(part as any).output"
              :error="(part as any).error"
              @confirm="handleConfirmAction(msg.id, pi)"
              @cancel="handleCancelAction(msg.id, pi)"
            />
          </template>
        </div>
      </div>

      </div>

      <!-- streaming indicator -->
      <LoadingIndicator v-if="loading" />
    </div>

    <!-- bottom bar -->
    <div class="ai-bottom">
      <div class="ai-suggestions">
        <span class="ai-suggestions-label">试试问</span>
        <button
          v-for="hint in DEFAULT_FOLLOWUPS"
          :key="hint"
          class="ai-hint"
          :disabled="!aiEnabled || loading"
          type="button"
          @click="handleSend(hint)"
        >
          {{ hint }}
        </button>
      </div>
      <div class="ai-input-bar">
        <input
          v-model="inputText"
          class="ai-input"
          :placeholder="aiEnabled ? '描述你想做什么...' : '登录后配置 BYOK 或升级 Pro 后可发送对话'"
          @keydown.enter="handleInputSend"
        />
        <button
          class="ai-send-btn"
          :disabled="!aiEnabled || !inputText.trim() || loading"
          @click="handleInputSend"
        >
          发送
        </button>
      </div>
    </div>

    <!-- footer -->
    <div v-if="messages.length > 0" class="ai-footer">
      <button class="ai-clear-btn" type="button" @click="clear">清空对话</button>
      <span class="ai-msg-count">{{ messages.filter(m => m.role === 'user').length }} 轮对话</span>
    </div>
  </div>
</template>

<style scoped>
.ai-chat { display: flex; flex-direction: column; height: 100%; background: transparent; }

.ai-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; flex: 1; padding: 40px 24px; text-align: center; }
.ai-empty-icon { color: var(--color-text-4); opacity: 0.3; margin-bottom: 12px; }
.ai-empty-title { font-size: 18px; font-weight: 600; color: var(--color-text-2); margin-bottom: 6px; }
.ai-pro-badge, .ai-send-pro { display: inline-flex; align-items: center; justify-content: center; border-radius: 999px; background: linear-gradient(135deg, #f59e0b, #f97316); color: #fff; font-weight: 700; line-height: 1; letter-spacing: 0; }
.ai-pro-badge { height: 16px; padding: 0 6px; font-size: 10px; vertical-align: middle; margin-left: 4px; }
.ai-send-pro { height: 14px; padding: 0 5px; font-size: 9px; margin-left: 2px; }
.ai-empty-desc { font-size: 13px; color: var(--color-text-4); margin-bottom: 20px; }
.ai-empty-hints { display: flex; flex-wrap: wrap; gap: 8px; justify-content: center; }
.ai-hint { padding: 6px 14px; font-size: 13px; color: var(--color-text-3); background: transparent; border: 1px solid rgba(var(--primary-6), 0.16); border-radius: 16px; cursor: pointer; transition: all 0.15s; font-family: inherit; }
.ai-hint:hover { color: rgb(var(--primary-6)); background: rgba(var(--primary-6), 0.08); border-color: rgba(var(--primary-6), 0.42); }
.ai-hint:disabled { opacity: 0.45; cursor: default; }
.ai-hint:disabled:hover { color: var(--color-text-3); background: transparent; border-color: rgba(var(--primary-6), 0.16); }

.ai-messages { flex: 1; overflow-y: auto; padding: 16px 48px; display: flex; flex-direction: column; }
.ai-msg { display: flex; gap: 10px; max-width: 85%; margin-bottom: 24px; }
.ai-msg:last-child { margin-bottom: 0; }
.ai-msg--user { align-self: flex-end; flex-direction: row-reverse; }
.ai-msg--assistant { align-self: flex-start; }
.ai-msg-avatar { display: flex; align-items: center; justify-content: center; width: 28px; height: 28px; border-radius: 8px; flex-shrink: 0; }
.ai-msg--user .ai-msg-avatar { background: rgb(var(--primary-6)); color: #fff; }
.ai-msg--assistant .ai-msg-avatar { background: var(--color-fill-2); color: var(--color-text-3); }
.ai-msg-body { min-width: 0; flex: 1; }
.ai-msg--user .ai-msg-body { text-align: right; }
.ai-msg-text { font-size: 14px; line-height: 1.65; color: var(--color-text-1); word-break: break-word; white-space: pre-wrap; }
.ai-msg--user .ai-msg-text { padding: 4px 0; text-align: right; }
.ai-msg--assistant .ai-msg-text { padding: 4px 0; }
.ai-msg-text :deep(strong) { font-weight: 600; }
.ai-msg-text :deep(em) { font-style: italic; }
.ai-msg-text :deep(del) { text-decoration: line-through; opacity: 0.7; }
.ai-msg-text :deep(code) { padding: 1px 5px; font-size: 12px; background: var(--color-fill-2); border-radius: 3px; font-family: 'SF Mono', 'Menlo', 'Monaco', monospace; }
.ai-msg-text :deep(a) { color: rgb(var(--primary-6)); text-decoration: underline; }
.ai-msg-text :deep(a:hover) { opacity: 0.8; }
.ai-msg-text :deep(h1), .ai-msg-text :deep(h2), .ai-msg-text :deep(h3), .ai-msg-text :deep(h4) { margin: 8px 0 4px; font-weight: 600; line-height: 1.3; }
.ai-msg-text :deep(h1) { font-size: 18px; }
.ai-msg-text :deep(h2) { font-size: 16px; }
.ai-msg-text :deep(h3) { font-size: 15px; }
.ai-msg-text :deep(h4) { font-size: 14px; }
.ai-msg-text :deep(ul), .ai-msg-text :deep(ol) { padding-left: 20px; margin: 4px 0; }
.ai-msg-text :deep(li) { margin: 2px 0; }
.ai-msg-text :deep(blockquote) { margin: 6px 0; padding: 4px 12px; border-left: 3px solid rgb(var(--primary-6)); color: var(--color-text-2); background: var(--color-fill-1); border-radius: 0 4px 4px 0; }
.ai-msg-text :deep(hr) { border: 0; border-top: 1px solid var(--color-border-2); margin: 10px 0; }
.ai-msg-text :deep(table) { width: 100%; border-collapse: collapse; margin: 8px 0; font-size: 13px; }
.ai-msg-text :deep(th), .ai-msg-text :deep(td) { padding: 6px 10px; border: 1px solid var(--color-border-2); text-align: left; }
.ai-msg-text :deep(th) { background: var(--color-fill-1); font-weight: 600; }
.ai-msg-text :deep(pre) { margin: 6px 0; padding: 10px 12px; background: var(--color-fill-1); border-radius: 6px; overflow-x: auto; font-size: 12px; line-height: 1.5; }
.ai-msg-text :deep(pre code) { padding: 0; background: transparent; font-size: inherit; }

.ai-bottom { flex-shrink: 0; padding: 0 48px 12px; background: transparent; }
.ai-suggestions { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; padding: 8px 0; }
.ai-suggestions-label { font-size: 12px; color: var(--color-text-4); flex-shrink: 0; margin-right: 2px; }
.ai-input-bar { display: flex; gap: 8px; }
.ai-input { flex: 1; padding: 10px 16px; font-size: 14px; color: var(--color-text-1); background: rgba(255, 255, 255, 0.06); border: 1px solid rgba(var(--primary-6), 0.14); border-radius: 10px; outline: none; font-family: inherit; }
.ai-input:focus { border-color: rgb(var(--primary-6)); box-shadow: 0 0 0 2px rgba(var(--primary-6), 0.12); }
.ai-input::placeholder { color: var(--color-text-4); }
.ai-send-btn { padding: 8px 20px; font-size: 14px; font-weight: 500; color: #fff; background: rgb(var(--primary-6)); border: 0; border-radius: 10px; cursor: pointer; font-family: inherit; transition: opacity 0.15s; }
.ai-send-btn:hover { opacity: 0.9; }
.ai-send-btn:disabled { opacity: 0.4; cursor: default; }

.ai-footer { display: flex; align-items: center; justify-content: space-between; padding: 8px 48px; border-top: 1px solid rgba(var(--primary-6), 0.12); background: transparent; }
.ai-clear-btn { padding: 4px 12px; font-size: 12px; color: var(--color-text-4); background: transparent; border: 0; border-radius: 4px; cursor: pointer; font-family: inherit; }
.ai-clear-btn:hover { color: rgb(var(--danger-6)); background: var(--color-fill-2); }
.ai-msg-count { font-size: 12px; color: var(--color-text-4); }

@media (max-width: 720px) { .ai-messages { padding: 16px; } .ai-footer { padding: 8px 16px; } }
</style>
