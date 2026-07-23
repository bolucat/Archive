<script setup lang="ts">
import { Bell, CheckCircle2, CircleAlert, LoaderCircle, RefreshCw, Trash2 } from 'lucide-vue-next'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import message from '../../utils/message'
import { t } from '../../i18n'
import { clearMediaAcquisitionNotifications, listMediaAcquisitionNotifications, markMediaAcquisitionNotificationsRead } from '../../services/mediaAcquisition/client'
import type { MediaAcquisitionNotification } from '@shared/types/mediaAcquisition'

const emit = defineEmits<{ (event: 'cleared'): void }>()
const notifications = ref<MediaAcquisitionNotification[]>([])
const loading = ref(false)
const clearing = ref(false)
const visibleUnreadIds = ref<Set<string>>(new Set())
let timer: ReturnType<typeof setInterval> | undefined
let unreadTimer: ReturnType<typeof setTimeout> | undefined

const groupedNotifications = computed(() => {
  const groups: Array<{ key: string; label: string; items: MediaAcquisitionNotification[] }> = []
  const map = new Map<string, { key: string; label: string; items: MediaAcquisitionNotification[] }>()
  for (const item of notifications.value) {
    const date = new Date(item.createdAt)
    const key = `${date.getFullYear()}-${date.getMonth() + 1}-${date.getDate()}`
    let group = map.get(key)
    if (!group) {
      group = { key, label: formatDateLabel(item.createdAt), items: [] }
      map.set(key, group)
      groups.push(group)
    }
    group.items.push(item)
  }
  return groups
})

async function refresh(markRead = false) {
  loading.value = true
  try {
    notifications.value = await listMediaAcquisitionNotifications(100)
    const unread = notifications.value.filter(item => !item.read).map(item => item.id)
    if (markRead && unread.length) {
      visibleUnreadIds.value = new Set(unread)
      await markMediaAcquisitionNotificationsRead(unread)
      notifications.value = notifications.value.map(item => ({ ...item, read: true }))
      if (unreadTimer) clearTimeout(unreadTimer)
      unreadTimer = setTimeout(() => {
        visibleUnreadIds.value = new Set()
      }, 12000)
    }
  } catch (error: any) {
    message.error(error?.message || t('ai.media.notifications.readFailed'))
  } finally {
    loading.value = false
  }
}

async function clearAll() {
  if (!notifications.value.length) return
  clearing.value = true
  try {
    const count = await clearMediaAcquisitionNotifications()
    notifications.value = []
    visibleUnreadIds.value = new Set()
    emit('cleared')
    message.success(t('ai.media.notifications.clearSuccess', { count }))
  } catch (error: any) {
    message.error(error?.message || t('ai.media.notifications.clearFailed'))
  } finally {
    clearing.value = false
  }
}

function formatTime(time: number) {
  return new Date(time).toLocaleString('zh-CN', { hour: '2-digit', minute: '2-digit', month: 'numeric', day: 'numeric' })
}

function formatDateLabel(time: number) {
  const date = new Date(time)
  const today = new Date()
  const yesterday = new Date(today)
  yesterday.setDate(today.getDate() - 1)
  if (date.toDateString() === today.toDateString()) return t('ai.media.notifications.today')
  if (date.toDateString() === yesterday.toDateString()) return t('ai.media.notifications.yesterday')
  return date.toLocaleDateString('zh-CN', { month: 'long', day: 'numeric', weekday: 'short' })
}

function hasUnreadDot(item: MediaAcquisitionNotification) {
  return !item.read || visibleUnreadIds.value.has(item.id)
}

function statusText(status: MediaAcquisitionNotification['status']) {
  return ({ completed: t('ai.media.status.completedLibrary'), partial: t('ai.media.status.partialLibrary'), no_coverage: t('ai.media.status.noCoverage'), failed: t('ai.media.status.failedAcquire'), cancelled: t('ai.media.status.cancelled') } as Record<MediaAcquisitionNotification['status'], string>)[status]
}

onMounted(() => {
  void refresh(true)
  timer = setInterval(() => void refresh(), 5000)
})

onBeforeUnmount(() => {
  if (timer) clearInterval(timer)
  if (unreadTimer) clearTimeout(unreadTimer)
})
</script>

<template>
  <section class="media-notifications-page">
    <header class="media-notifications-head">
      <div><h2>{{ t('ai.media.notifications.title') }}</h2><p>{{ t('ai.media.notifications.subtitle') }}</p></div>
      <div class="media-notifications-actions">
        <button type="button" class="media-notifications-clear" :disabled="clearing || !notifications.length" :title="t('ai.media.notifications.clearAllTitle')" @click="clearAll"><Trash2 :size="15" />{{ t('common.clear') }}</button>
        <button type="button" :disabled="loading" :title="t('ai.media.notifications.refreshTitle')" @click="refresh()"><LoaderCircle v-if="loading" :size="16" class="spin" /><RefreshCw v-else :size="16" /></button>
      </div>
    </header>
    <div v-if="!notifications.length && !loading" class="media-notifications-empty">
      <Bell :size="32" stroke-width="1.7" />
      <strong>{{ t('ai.media.notifications.emptyTitle') }}</strong>
      <span>{{ t('ai.media.notifications.emptyDesc') }}</span>
    </div>
    <div v-else class="media-notifications-list">
      <section v-for="group in groupedNotifications" :key="group.key" class="media-notification-day">
        <div class="media-notification-day-head"><span>{{ group.label }}</span><em>{{ group.items.length }} {{ t('ai.media.notifications.itemsUnit') }}</em></div>
        <article v-for="item in group.items" :key="item.id" :class="['media-notification', { unread: hasUnreadDot(item), failed: item.status === 'failed', warning: item.status === 'partial' || item.status === 'no_coverage', cancelled: item.status === 'cancelled' }]">
          <span v-if="hasUnreadDot(item)" class="media-notification-dot" />
          <CheckCircle2 v-if="item.status === 'completed'" :size="20" />
          <CircleAlert v-else :size="20" />
          <div><strong>{{ item.title }}</strong><p>{{ statusText(item.status) }} · {{ item.message }}</p></div>
          <time>{{ formatTime(item.createdAt) }}</time>
        </article>
      </section>
    </div>
  </section>
</template>

<style scoped>
.media-notifications-page { height: 100%; overflow: auto; padding: 32px; color: var(--color-text-1); background: var(--color-bg-1); }.media-notifications-head { display: flex; justify-content: space-between; gap: 24px; padding-bottom: 24px; }.media-notifications-head h2 { margin: 0 0 7px; font-size: 32px; line-height: 1.1; }.media-notifications-head p { margin: 0; color: var(--color-text-3); font-size: 14px; }.media-notifications-actions { display: flex; gap: 8px; }.media-notifications-head button { width: 34px; height: 34px; border: 1px solid var(--color-border-2); border-radius: 6px; color: var(--color-text-2); background: transparent; cursor: pointer; }.media-notifications-head button:disabled { cursor: not-allowed; opacity: .45; }.media-notifications-head .media-notifications-clear { display: inline-flex; width: auto; align-items: center; gap: 5px; padding: 0 10px; }.media-notifications-empty { min-height: calc(100% - 130px); display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 14px; color: var(--color-text-3); text-align: center; transform: translateY(-8%); }.media-notifications-empty svg { color: var(--color-text-3); opacity: .9; }.media-notifications-empty strong { color: var(--color-text-1); font-size: 18px; font-weight: 700; }.media-notifications-empty span { color: var(--color-text-3); font-size: 14px; }.media-notifications-list { display: grid; gap: 22px; max-width: 900px; padding-top: 22px; }.media-notification-day { display: grid; gap: 9px; }.media-notification-day-head { display: flex; align-items: center; justify-content: space-between; color: var(--color-text-2); font-size: 13px; font-weight: 700; }.media-notification-day-head em { color: var(--color-text-4); font-style: normal; font-weight: 500; }.media-notification { position: relative; display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 12px; padding: 14px 16px; border: 1px solid var(--color-border-2); border-radius: 7px; color: rgb(var(--success-6)); background: var(--color-bg-2); }.media-notification.unread { border-color: color-mix(in srgb, rgb(var(--danger-6)) 68%, var(--color-border-2)); background: color-mix(in srgb, var(--color-bg-2) 92%, rgb(var(--danger-6)) 8%); }.media-notification-dot { position: absolute; top: 12px; right: 12px; width: 8px; height: 8px; border-radius: 50%; background: rgb(var(--danger-6)); box-shadow: 0 0 0 3px color-mix(in srgb, rgb(var(--danger-6)) 18%, transparent); }.media-notification.failed { color: rgb(var(--danger-6)); }.media-notification.warning { color: rgb(var(--warning-6)); }.media-notification.cancelled { color: var(--color-text-3); }.media-notification strong { color: var(--color-text-1); }.media-notification p { margin: 4px 0 0; color: var(--color-text-3); font-size: 12px; }.media-notification time { color: var(--color-text-3); font-size: 11px; white-space: nowrap; }.spin { animation: notification-spin .8s linear infinite; }@keyframes notification-spin { to { transform: rotate(360deg); } }
</style>
