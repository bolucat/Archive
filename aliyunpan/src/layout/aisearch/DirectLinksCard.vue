<script setup lang="ts">
import { AlertCircle, CheckCircle, Link2, Loader2 } from 'lucide-vue-next'

defineProps<{
  state: 'running' | 'done' | 'error'
  input?: { format: 'url' | 'aria2'; files: { name: string }[] }
  output?: { total: number; success: number; failed: number; text: string }
  error?: string
}>()
</script>

<template>
  <div class="dl-card">
    <div v-if="state === 'running'" class="dl-status">
      <Loader2 :size="14" :stroke-width="2" class="dl-spin" />
      <span>正在导出 {{ input?.files?.length || 0 }} 个选中项的直链...</span>
    </div>

    <div v-else-if="state === 'done'" class="dl-result">
      <CheckCircle :size="16" :stroke-width="1.5" />
      <div class="dl-body">
        <div>直链导出完成：{{ output?.success || 0 }}/{{ output?.total || 0 }}{{ output?.failed ? `，${output.failed} 失败` : '' }}</div>
        <textarea class="dl-textarea" :value="output?.text || ''" readonly />
      </div>
    </div>

    <div v-else class="dl-status dl-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>{{ error || '直链导出失败' }}</span>
    </div>

    <div v-if="state === 'done'" class="dl-footer">
      <Link2 :size="13" :stroke-width="1.5" />
      <span>{{ input?.format === 'aria2' ? 'aria2 格式' : 'URL 列表' }}</span>
    </div>
  </div>
</template>

<style scoped>
.dl-card { margin: 8px 0; border-radius: 8px; background: var(--color-fill-1); border: 1px solid var(--color-border-2); overflow: hidden; }
.dl-status, .dl-result { display: flex; gap: 8px; align-items: flex-start; padding: 12px 14px; font-size: 13px; color: var(--color-text-2); }
.dl-result { color: rgb(var(--success-6)); }
.dl-body { flex: 1; min-width: 0; }
.dl-textarea { width: 100%; min-height: 150px; margin-top: 8px; padding: 8px; border: 1px solid var(--color-border-2); border-radius: 6px; background: var(--color-bg-2); color: var(--color-text-2); font-size: 12px; line-height: 1.5; resize: vertical; box-sizing: border-box; }
.dl-error { color: rgb(var(--danger-6)); }
.dl-footer { display: flex; align-items: center; gap: 6px; padding: 8px 14px; border-top: 1px solid var(--color-border-2); color: var(--color-text-4); font-size: 12px; }
.dl-spin { animation: dl-spin 1s linear infinite; }
@keyframes dl-spin { to { transform: rotate(360deg); } }
</style>
