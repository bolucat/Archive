<script setup lang="ts">
import { Loader2, CheckCircle, AlertCircle } from 'lucide-vue-next'

defineProps<{
  state: 'parsing' | 'listing' | 'saving' | 'done' | 'error'
  input?: { url: string; password: string }
  output?: { shareName: string; fileCount: number; savedCount: number; platform: string; asyncStatus?: boolean }
  error?: string
}>()

const emit = defineEmits<{ (e: 'retry'): void }>()
</script>

<template>
  <div class="is-card">
    <div v-if="state === 'parsing'" class="is-status"><Loader2 :size="14" :stroke-width="2" class="is-spin" /> 解析分享链接...</div>
    <div v-else-if="state === 'listing'" class="is-status"><Loader2 :size="14" :stroke-width="2" class="is-spin" /> 列出分享文件...</div>
    <div v-else-if="state === 'saving'" class="is-status"><Loader2 :size="14" :stroke-width="2" class="is-spin" /> 正在转存到你的网盘...</div>
    <div v-else-if="state === 'done' && output && output.asyncStatus" class="is-status is-done">
      <CheckCircle :size="16" :stroke-width="1.5" />
      <span>已从 {{ output.platform }} 分享「{{ output.shareName }}」提交转存 {{ output.fileCount }} 个文件，后台处理中</span>
    </div>
    <div v-else-if="state === 'done' && output" class="is-status is-done">
      <CheckCircle :size="16" :stroke-width="1.5" />
      <span>已从 {{ output.platform }} 分享「{{ output.shareName }}」转存 {{ output.savedCount }}/{{ output.fileCount }} 个文件</span>
    </div>
    <div v-else-if="state === 'error'" class="is-status is-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>{{ error || '导入失败' }}</span>
      <button class="is-retry" @click="emit('retry')">重试</button>
    </div>
  </div>
</template>

<style scoped>
.is-card { margin: 12px 0; border-radius: 13px; background: color-mix(in srgb, var(--color-fill-1) 78%, transparent); border: 1px solid color-mix(in srgb, var(--color-border-2) 88%, transparent); overflow: hidden; box-shadow: 0 10px 26px rgba(0, 0, 0, .06); }
.is-status { display: flex; align-items: center; gap: 8px; padding: 12px 14px; font-size: 13px; color: var(--color-text-3); }
.is-done { color: rgb(var(--success-6)); }
.is-error { color: rgb(var(--danger-6)); }
.is-retry { margin-left: auto; padding: 4px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: rgba(var(--primary-6), .06); border: 1px solid rgba(var(--primary-6), .3); border-radius: 7px; cursor: pointer; }
.is-spin { animation: is-spin 1s linear infinite; }
@keyframes is-spin { to { transform: rotate(360deg); } }
</style>
