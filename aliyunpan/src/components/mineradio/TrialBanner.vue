<script setup lang="ts">
/**
 * TrialBanner — contextual notification banner.
 * Shows when music library is empty or needs a scan.
 * Dismissible, with action buttons.
 */
import { ref, watch } from 'vue'
import { Music2, X } from 'lucide-vue-next'

const props = defineProps<{
  trackCount: number
  isScanning: boolean
  lastScanAt: number
}>()

const emit = defineEmits<{ (e: 'start-scan'): void; (e: 'import-local-songs'): void; (e: 'dismiss'): void }>()

const show = ref(false)
const message = ref('')
const actionLabel = ref('')
const secondaryLabel = ref('')
const state = ref<'empty' | 'scanning' | 'scan-complete' | 'first-play' | ''>('')

const DISMISS_PREFIX = 'boxplayer-mineradio-banner-dismissed.'

function dismissKey(nextState: string) {
  return `${DISMISS_PREFIX}${nextState}`
}

function updateBanner() {
  const now = Date.now()
  let nextState: typeof state.value = ''
  let nextMessage = ''
  let nextAction = ''
  let nextSecondary = ''
  if (props.trackCount === 0 && !props.isScanning) {
    nextState = 'empty'
    nextMessage = '音乐库为空 — 扫描网盘音乐，或临时导入本地歌曲'
    nextAction = '开始扫描'
    nextSecondary = '导入本地'
  } else if (props.isScanning) {
    nextState = 'scanning'
    nextMessage = '正在扫描网盘音乐 · 发现的歌曲会自动进入音乐库'
  } else if (props.lastScanAt && now - props.lastScanAt < 2 * 60 * 1000) {
    nextState = 'scan-complete'
    nextMessage = `扫描完成 · 已收录 ${props.trackCount} 首网盘音乐`
    nextSecondary = '导入本地'
  } else if (props.trackCount > 0 && !props.lastScanAt) {
    nextState = 'first-play'
    nextMessage = `已收录 ${props.trackCount} 首网盘音乐 · 点击任意歌曲开始播放`
    nextSecondary = '导入本地'
  }

  if (!nextState || localStorage.getItem(dismissKey(nextState))) {
    show.value = false
    state.value = ''
    return
  }
  state.value = nextState
  message.value = nextMessage
  actionLabel.value = nextAction
  secondaryLabel.value = nextSecondary
  show.value = true
}

watch(() => [props.trackCount, props.isScanning, props.lastScanAt] as const, updateBanner, { immediate: true })

function onAction() {
  if (!props.trackCount) emit('start-scan')
  show.value = false
}

function onSecondary() {
  emit('import-local-songs')
  show.value = false
}

function onDismiss() {
  show.value = false
  if (state.value) localStorage.setItem(dismissKey(state.value), '1')
  emit('dismiss')
}
</script>

<template>
  <div v-if="show" class="trial-banner">
    <Music2 :size="14" :stroke-width="2" class="trial-icon" />
    <span class="trial-text">{{ message }}</span>
    <button v-if="actionLabel" class="trial-action" @click="onAction">{{ actionLabel }}</button>
    <button v-if="secondaryLabel" class="trial-action ghost" @click="onSecondary">{{ secondaryLabel }}</button>
    <button class="trial-close" @click="onDismiss"><X :size="14" /></button>
  </div>
</template>

<style scoped lang="less">
.trial-banner {
  display: flex; align-items: center; gap: 10px;
  padding: 8px 14px; margin-bottom: 12px;
  border-radius: 12px;
  border: 1px solid rgba(0,245,212,.14);
  background: rgba(0,245,212,.045);
  color: rgba(255,255,255,.7);
  font-size: 12px;
}
.trial-icon { color: rgba(0,245,212,.7); flex-shrink: 0; }
.trial-text { flex: 1; }
.trial-action {
  height: 26px; padding: 0 10px; border-radius: 8px;
  border: 1px solid rgba(0,245,212,.3); background: rgba(0,245,212,.1);
  color: #fff; font-family: inherit; font-size: 11px; cursor: pointer; font-weight: 650;
}
.trial-action:hover { background: rgba(0,245,212,.18); }
.trial-action.ghost {
  border-color: rgba(244,210,138,.24);
  background: rgba(244,210,138,.075);
  color: rgba(255,240,196,.92);
}
.trial-action.ghost:hover { background: rgba(244,210,138,.13); }
.trial-close {
  width: 22px; height: 22px; border: none; border-radius: 5px;
  background: rgba(255,255,255,.06); color: rgba(255,255,255,.4); cursor: pointer;
  display: flex; align-items: center; justify-content: center;
}
.trial-close:hover { background: rgba(255,255,255,.12); color: #fff; }
</style>
