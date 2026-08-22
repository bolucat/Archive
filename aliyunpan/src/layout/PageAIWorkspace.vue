<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { ClipboardCheck, FileText, FolderSearch, ShieldCheck, Sparkles } from 'lucide-vue-next'
import { useAppStore } from '../store'
import { getAIConfig, migrateSoleSavedBYOKAsDefault } from '../utils/bookAI'
import { isBoxPlayerCloudProvider } from '../utils/boxplayerCloudAI'
import { isLoggedIn, isPro } from '../utils/usageLimit'
import WorkspaceAgent from './WorkspaceAgent.vue'
import AISearchAgent from './AISearchAgent.vue'
import DocumentAIModal from '../components/DocumentAIModal.vue'
import MediaAcquisitionNotifications from './aisearch/MediaAcquisitionNotifications.vue'
import MediaAcquisitionTasks from './aisearch/MediaAcquisitionTasks.vue'
import MediaAcquisitionTracking from './aisearch/MediaAcquisitionTracking.vue'
import { listMediaAcquisitionNotifications, listMediaAcquisitionRuns, listMediaAcquisitionTracking } from '../services/mediaAcquisition/client'
import type { DocumentInsightLaunchContext } from '../services/documents/insight'
import { t } from '../i18n'

const appStore = useAppStore()
const props = withDefaults(defineProps<{ sidebarVisible?: boolean }>(), { sidebarVisible: true })
migrateSoleSavedBYOKAsDefault()
const documentContext = ref<DocumentInsightLaunchContext | null>(null)
const activeView = ref<'chat' | 'mediaTasks' | 'tracking' | 'notifications'>('chat')
const workspaceMode = ref<'agent' | 'documents' | 'plans'>('agent')
const activeTaskCount = ref(0)
const trackingCount = ref(0)
const unreadNotificationCount = ref(0)
let badgeTimer: ReturnType<typeof setInterval> | undefined
const aiEnabled = computed(() => {
  if (isPro()) return true
  const config = getAIConfig()
  return isLoggedIn() && !!config && !isBoxPlayerCloudProvider(config.providerName)
})

function openPanHubSearch(keyword: string) {
  sessionStorage.setItem('boxplayer:pending-panhub-search', keyword)
  window.dispatchEvent(new CustomEvent('boxplayer:open-panhub-search', { detail: { keyword } }))
  appStore.toggleTab('search')
}

function consumeDocumentContext() {
  try {
    const raw = sessionStorage.getItem('boxplayer:pending-document-ai')
    if (!raw) return
    const parsed = JSON.parse(raw) as DocumentInsightLaunchContext & { file?: any; userId?: string }
    const sources = parsed.sources?.length ? parsed.sources : (parsed.file?.file_id && parsed.userId ? [{ file: parsed.file, userId: parsed.userId }] : [])
    if (!sources.length) return
    documentContext.value = { ...parsed, sources }
    sessionStorage.removeItem('boxplayer:pending-document-ai')
  } catch {
    sessionStorage.removeItem('boxplayer:pending-document-ai')
  }
}

function onOpenDocumentAI() {
  consumeDocumentContext()
  workspaceMode.value = 'documents'
  activeView.value = 'chat'
}
function onOpenMediaTasks() {
  activeView.value = 'mediaTasks'
  sessionStorage.removeItem('boxplayer:open-media-tasks')
}

async function refreshBadges() {
  try {
    const [runs, tracking, notifications] = await Promise.all([
      listMediaAcquisitionRuns(80),
      listMediaAcquisitionTracking(100),
      listMediaAcquisitionNotifications(100)
    ])
    activeTaskCount.value = runs.filter(run => ['queued', 'searching', 'selecting', 'transferring', 'verifying', 'organizing', 'retry_wait'].includes(run.status)).length
    trackingCount.value = tracking.filter(item => item.status !== 'ended' && item.status !== 'complete').length
    unreadNotificationCount.value = notifications.filter(item => !item.read).length
  } catch {
    activeTaskCount.value = 0
    trackingCount.value = 0
    unreadNotificationCount.value = 0
  }
}

onMounted(() => {
  consumeDocumentContext()
  if (sessionStorage.getItem('boxplayer:open-media-tasks') === '1') onOpenMediaTasks()
  void refreshBadges()
  badgeTimer = setInterval(() => void refreshBadges(), 5000)
  window.addEventListener('boxplayer:open-document-ai', onOpenDocumentAI)
  window.addEventListener('boxplayer:open-media-tasks', onOpenMediaTasks)
})

onBeforeUnmount(() => {
  if (badgeTimer) clearInterval(badgeTimer)
  window.removeEventListener('boxplayer:open-document-ai', onOpenDocumentAI)
  window.removeEventListener('boxplayer:open-media-tasks', onOpenMediaTasks)
})
</script>

<template>
  <div class="ai-workspace-page">
    <div class="ai-workspace-view-switcher">
      <button type="button" :title="t('ai.workspace')" :class="{ active: activeView === 'chat' }" @click="activeView = 'chat'">{{ t('ai.workspace') }}</button>
      <button type="button" :title="t('ai.activities')" :class="{ active: activeView === 'mediaTasks' }" @click="activeView = 'mediaTasks'">{{ t('ai.activities') }}<span v-if="activeTaskCount" class="workspace-badge">{{ activeTaskCount }}</span></button>
      <button type="button" :title="t('ai.tracking')" :class="{ active: activeView === 'tracking' }" @click="activeView = 'tracking'">{{ t('ai.tracking') }}<span v-if="trackingCount" class="workspace-badge">{{ trackingCount }}</span></button>
      <button type="button" :title="t('ai.notifications')" :class="{ active: activeView === 'notifications' }" @click="activeView = 'notifications'">{{ t('ai.notifications') }}<span v-if="unreadNotificationCount" class="workspace-badge unread">{{ unreadNotificationCount }}</span></button>
    </div>
    <div :class="['ai-workspace-content', { 'ai-workspace-content--chat': activeView === 'chat' }]">
      <section v-show="activeView === 'chat'" class="ai-workspace-shell">
        <nav class="ai-workspace-mode-switcher" aria-label="AI workspace mode">
          <button type="button" :class="{ active: workspaceMode === 'agent' }" @click="workspaceMode = 'agent'">AI 助手</button>
          <button type="button" :class="{ active: workspaceMode === 'documents' }" @click="workspaceMode = 'documents'">文档洞察</button>
          <button type="button" :class="{ active: workspaceMode === 'plans' }" @click="workspaceMode = 'plans'">可审查计划</button>
        </nav>
        <AISearchAgent v-show="workspaceMode === 'agent'" :ai-enabled="aiEnabled" :sidebar-visible="props.sidebarVisible" @search-resource="openPanHubSearch" />
        <div v-show="workspaceMode === 'documents'" class="ai-workspace-document-state">
          <DocumentAIModal v-if="documentContext" :visible="!!documentContext" :available-sources="documentContext.availableSources || []" :initial-prompt="documentContext.initialPrompt || ''" mode="workspace" :scope-name="documentContext.scopeName || ''" :sources="documentContext.sources || []" @update:visible="visible => { if (!visible) documentContext = null }" />
          <section v-else class="document-insight-home">
            <span class="document-insight-home__icon"><Sparkles :size="27" /></span>
            <p class="document-insight-home__eyebrow">BoxPlayer AI · 只读文档洞察</p>
            <h1>从一个网盘文档开始</h1>
            <p>在网盘中选择支持的文件，再使用 <strong>BoxPlayer AI → AI 问答 / AI 摘要</strong>。文档只用于本次检索和回答，不会产生审批或写入操作。</p>
            <div class="document-insight-home__steps">
              <article><FileText :size="18" /><strong>选择来源</strong><span>PDF、DOCX、EPUB、TXT 或 Markdown，最多 10 份。</span></article>
              <article><FolderSearch :size="18" /><strong>提出问题</strong><span>按页码或片段检索，并显示可追溯引用。</span></article>
              <article><ShieldCheck :size="18" /><strong>保持只读</strong><span>文档会话不会移动、删除、分享或下载文件。</span></article>
            </div>
            <div class="document-insight-home__actions"><button type="button" class="document-insight-home__open-pan" @click="appStore.toggleTab('pan')"><FolderSearch :size="16" />打开网盘选择文档</button><button type="button" class="document-insight-home__plan-link" @click="workspaceMode = 'plans'"><ClipboardCheck :size="16" />需要整理网盘？生成可审查计划</button></div>
          </section>
        </div>
        <WorkspaceAgent v-show="workspaceMode === 'plans'" :ai-enabled="aiEnabled" :sidebar-visible="props.sidebarVisible" @back-to-documents="workspaceMode = 'documents'" />
      </section>
      <MediaAcquisitionTasks v-if="activeView === 'mediaTasks'" />
      <MediaAcquisitionTracking v-else-if="activeView === 'tracking'" />
      <MediaAcquisitionNotifications v-else-if="activeView === 'notifications'" @cleared="refreshBadges" />
    </div>
  </div>
</template>

<style scoped>
.ai-workspace-page { position: relative; display: flex; flex-direction: column; box-sizing: border-box; height: 100%; min-height: 0; gap: 14px; padding: 18px; overflow: hidden; background: transparent; }
.ai-workspace-content { flex: 1; min-height: 0; overflow: hidden; border: 1px solid var(--app-glass-line); border-radius: 24px; background: var(--app-glass-panel); box-shadow: 0 20px 60px rgba(0,0,0,.22), inset 0 1px 0 rgba(255,255,255,.075); backdrop-filter: blur(24px) saturate(1.14); }
.ai-workspace-content--chat { border: 0; border-radius: 0; background: transparent; box-shadow: none; backdrop-filter: none; }
.ai-workspace-shell { display:flex; height:100%; min-height:0; flex-direction:column; gap:10px; }.ai-workspace-mode-switcher { display:flex; align-self:center; gap:3px; padding:3px; border:1px solid var(--color-border-2); border-radius:9px; background:var(--color-bg-2); }.ai-workspace-mode-switcher button { min-width:86px; padding:6px 10px; border:0; border-radius:6px; color:var(--color-text-2); background:transparent; cursor:pointer; font:inherit; font-size:12px; font-weight:650; }.ai-workspace-mode-switcher button.active { color:var(--color-text-1); background:var(--color-fill-2); }
.ai-workspace-document-state { min-height:0; flex:1; overflow:hidden; }.document-insight-home { display:flex; height:100%; min-height:0; box-sizing:border-box; flex-direction:column; align-items:center; justify-content:center; padding:38px; color:var(--color-text-2); text-align:center; }.document-insight-home__icon { display:grid; width:58px; height:58px; place-items:center; margin-bottom:17px; border-radius:18px; color:#fff; background:linear-gradient(135deg,#ff3fd8,#795cff 49%,#38a7ff); box-shadow:0 12px 28px rgba(104,84,233,.25); }.document-insight-home__eyebrow { margin:0 0 6px; color:#8067eb; font-size:12px; font-weight:700; letter-spacing:.03em; }.document-insight-home h1 { margin:0; color:var(--color-text-1); font-size:23px; letter-spacing:-.4px; }.document-insight-home>p:not(.document-insight-home__eyebrow) { max-width:570px; margin:10px 0 24px; font-size:13px; line-height:1.75; }.document-insight-home__steps { display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); width:min(760px,100%); gap:10px; text-align:left; }.document-insight-home__steps article { display:grid; gap:6px; padding:15px; border:1px solid var(--color-border-2); border-radius:11px; background:var(--color-bg-2); }.document-insight-home__steps svg { color:#7964ea; }.document-insight-home__steps strong { color:var(--color-text-1); font-size:13px; }.document-insight-home__steps span { font-size:11px; line-height:1.55; }.document-insight-home__actions { display:flex; flex-wrap:wrap; justify-content:center; gap:9px; margin-top:20px; }.document-insight-home__actions button { display:inline-flex; align-items:center; gap:7px; padding:9px 13px; border-radius:8px; cursor:pointer; font:inherit; font-size:12px; font-weight:650; }.document-insight-home__open-pan { border:0; color:#fff; background:linear-gradient(100deg,#5d5cff,#806eff); }.document-insight-home__plan-link { border:1px solid rgba(122,100,234,.45); color:#735cde; background:transparent; }
.ai-workspace-view-switcher { display: flex; align-self: center; max-width: 100%; height: 38px; min-height: 38px; align-items: center; gap: 5px; overflow-x: auto; overflow-y: hidden; padding: 3px; border: 1px solid var(--color-border-2); border-radius: 10px; background: var(--color-bg-2); scrollbar-width: none; }
.ai-workspace-view-switcher::-webkit-scrollbar { display: none; }
.ai-workspace-view-switcher button { display: inline-flex; flex: 0 0 auto; max-width: 108px; height: 30px; align-items: center; justify-content: center; gap: 6px; overflow: hidden; padding: 0 14px; border: 0; border-radius: 7px; color: #fff; background: transparent; cursor: pointer; font-size: 14px; font-weight: 650; line-height: 1; text-overflow: ellipsis; white-space: nowrap; }
.ai-workspace-view-switcher button.active { color: var(--color-text-1); background: var(--color-fill-2); }
.workspace-badge { display: inline-flex; min-width: 18px; height: 18px; align-items: center; justify-content: center; padding: 0 5px; border-radius: 999px; color: var(--color-text-1); background: var(--color-fill-3); font-size: 11px; line-height: 1; }
.workspace-badge.unread { color: #fff; background: rgb(var(--danger-6)); }
@media (max-width:700px) { .document-insight-home__steps { grid-template-columns:1fr; }.document-insight-home { justify-content:flex-start; overflow:auto; } }
</style>

<style>
body:not([arco-theme='dark']) .ai-workspace-view-switcher button {
  color: #111827;
}
</style>
