<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useAppStore } from '../store'
import { getAIConfig } from '../utils/bookAI'
import { isBoxPlayerCloudProvider } from '../utils/boxplayerCloudAI'
import { isLoggedIn, isPro } from '../utils/usageLimit'
import AISearchAgent from './AISearchAgent.vue'
import MediaAcquisitionNotifications from './aisearch/MediaAcquisitionNotifications.vue'
import MediaAcquisitionTasks from './aisearch/MediaAcquisitionTasks.vue'
import MediaAcquisitionTracking from './aisearch/MediaAcquisitionTracking.vue'
import { listMediaAcquisitionNotifications, listMediaAcquisitionRuns, listMediaAcquisitionTracking } from '../services/mediaAcquisition/client'
import type { WorkspaceDocumentContext } from './aisearch/useAISearchChat'

const appStore = useAppStore()
const documentContext = ref<WorkspaceDocumentContext | null>(null)
const activeView = ref<'chat' | 'mediaTasks' | 'tracking' | 'notifications'>('chat')
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
    const context = JSON.parse(raw) as WorkspaceDocumentContext
    if (!context?.file?.file_id || !context.userId) return
    documentContext.value = context
    sessionStorage.removeItem('boxplayer:pending-document-ai')
  } catch {
    sessionStorage.removeItem('boxplayer:pending-document-ai')
  }
}

function onOpenDocumentAI() { consumeDocumentContext() }
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
      <button type="button" :class="{ active: activeView === 'chat' }" @click="activeView = 'chat'">智能工作台</button>
      <button type="button" :class="{ active: activeView === 'mediaTasks' }" @click="activeView = 'mediaTasks'">活动<span v-if="activeTaskCount" class="workspace-badge">{{ activeTaskCount }}</span></button>
      <button type="button" :class="{ active: activeView === 'tracking' }" @click="activeView = 'tracking'">追更<span v-if="trackingCount" class="workspace-badge">{{ trackingCount }}</span></button>
      <button type="button" :class="{ active: activeView === 'notifications' }" @click="activeView = 'notifications'">通知<span v-if="unreadNotificationCount" class="workspace-badge unread">{{ unreadNotificationCount }}</span></button>
    </div>
    <div class="ai-workspace-content with-view-switcher">
      <AISearchAgent v-if="activeView === 'chat'" :ai-enabled="aiEnabled" :document-context="documentContext" @search-resource="openPanHubSearch" />
      <MediaAcquisitionTasks v-else-if="activeView === 'mediaTasks'" />
      <MediaAcquisitionTracking v-else-if="activeView === 'tracking'" />
      <MediaAcquisitionNotifications v-else @cleared="refreshBadges" />
    </div>
  </div>
</template>

<style scoped>
.ai-workspace-page { position: relative; height: 100%; min-height: 0; overflow: hidden; background: var(--color-bg-1); }
.ai-workspace-content { height: 100%; min-height: 0; }
.ai-workspace-content.with-view-switcher { box-sizing: border-box; padding-top: 50px; }
.ai-workspace-view-switcher { position: absolute; z-index: 3; top: 14px; right: 22px; display: flex; max-width: calc(100% - 44px); height: 30px; min-height: 30px; align-items: center; gap: 4px; overflow-x: auto; overflow-y: hidden; padding: 3px; border: 1px solid var(--color-border-2); border-radius: 7px; background: var(--color-bg-2); scrollbar-width: none; }
.ai-workspace-view-switcher::-webkit-scrollbar { display: none; }
.ai-workspace-view-switcher button { display: inline-flex; flex: 0 0 auto; height: 22px; align-items: center; justify-content: center; gap: 5px; padding: 0 9px; border: 0; border-radius: 4px; color: var(--color-text-3); background: transparent; cursor: pointer; font-size: 12px; line-height: 1; white-space: nowrap; }
.ai-workspace-view-switcher button.active { color: var(--color-text-1); background: var(--color-fill-2); }
.workspace-badge { display: inline-flex; min-width: 16px; height: 16px; align-items: center; justify-content: center; padding: 0 5px; border-radius: 999px; color: var(--color-text-1); background: var(--color-fill-3); font-size: 10px; line-height: 1; }
.workspace-badge.unread { color: #fff; background: rgb(var(--danger-6)); }
</style>
