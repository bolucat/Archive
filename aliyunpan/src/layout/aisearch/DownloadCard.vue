<script setup lang="ts">
import { Loader2, CheckCircle, AlertCircle } from 'lucide-vue-next'

defineProps<{
  state: 'running' | 'done' | 'error'
  input?: { files: { name: string; fileId: string; driveId: string; userId: string }[] }
  output?: { total: number; success: number }
  error?: string
}>()

const emit = defineEmits<{ (e: 'retry'): void }>()
</script>

<template>
  <div class="dl-card">
    <div v-if="state === 'running'" class="dl-status">
      <Loader2 :size="14" :stroke-width="2" class="dl-spin" />
      <span>正在添加 {{ input?.files.length || 0 }} 个下载任务...</span>
    </div>
    <div v-else-if="state === 'done' && output" class="dl-status dl-done">
      <CheckCircle :size="16" :stroke-width="1.5" />
      <span>已添加 {{ output.success }}/{{ output.total }} 个下载任务，请在下载页面查看进度</span>
    </div>
    <div v-else-if="state === 'error'" class="dl-status dl-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>{{ error || '添加下载失败' }}</span>
      <button class="dl-retry" @click="emit('retry')">重试</button>
    </div>
  </div>
</template>

<style scoped>
.dl-card { margin: 12px 0; border-radius: 13px; background: color-mix(in srgb, var(--color-fill-1) 78%, transparent); border: 1px solid color-mix(in srgb, var(--color-border-2) 88%, transparent); overflow: hidden; box-shadow: 0 10px 26px rgba(0, 0, 0, .06); }
.dl-status { display: flex; align-items: center; gap: 8px; padding: 12px 14px; font-size: 13px; color: var(--color-text-3); }
.dl-done { color: rgb(var(--success-6)); }
.dl-error { color: rgb(var(--danger-6)); }
.dl-retry { margin-left: auto; padding: 4px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: rgba(var(--primary-6), .06); border: 1px solid rgba(var(--primary-6), .3); border-radius: 7px; cursor: pointer; }
.dl-spin { animation: dl-spin 1s linear infinite; }
@keyframes dl-spin { to { transform: rotate(360deg); } }
</style>
