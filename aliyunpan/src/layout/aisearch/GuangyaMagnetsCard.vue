<script setup lang="ts">
import { AlertCircle, CheckCircle, Loader2, Magnet } from 'lucide-vue-next'

defineProps<{
  state: 'confirm' | 'running' | 'done' | 'error'
  input?: { magnets: string[]; parentId?: string }
  output?: { total: number; success: number; failed: number; report: string }
  error?: string
}>()

const emit = defineEmits<{ (e: 'confirm'): void; (e: 'cancel'): void }>()
</script>

<template>
  <div class="gm-card">
    <template v-if="state === 'confirm'">
      <div class="gm-header">
        <Magnet :size="16" :stroke-width="1.5" />
        <span>确认提交 {{ input?.magnets?.length || 0 }} 条磁力到光鸭云添加？</span>
      </div>
      <div class="gm-list">
        <div v-for="(magnet, index) in (input?.magnets || []).slice(0, 8)" :key="index" class="gm-item">{{ magnet }}</div>
        <div v-if="(input?.magnets?.length || 0) > 8" class="gm-more">…还有 {{ (input?.magnets?.length || 0) - 8 }} 条</div>
      </div>
      <div class="gm-actions">
        <button class="gm-btn gm-btn-primary" @click="emit('confirm')">确认提交</button>
        <button class="gm-btn" @click="emit('cancel')">取消</button>
      </div>
    </template>

    <div v-else-if="state === 'running'" class="gm-status">
      <Loader2 :size="14" :stroke-width="2" class="gm-spin" />
      <span>正在提交光鸭云添加任务...</span>
    </div>

    <div v-else-if="state === 'done'" class="gm-result">
      <CheckCircle :size="16" :stroke-width="1.5" />
      <pre>{{ output?.report || `提交完成：${output?.success || 0}/${output?.total || 0}` }}</pre>
    </div>

    <div v-else class="gm-status gm-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>{{ error || '磁力云添加失败' }}</span>
    </div>
  </div>
</template>

<style scoped>
.gm-card { margin: 12px 0; border-radius: 13px; background: color-mix(in srgb, var(--color-fill-1) 78%, transparent); border: 1px solid color-mix(in srgb, var(--color-border-2) 88%, transparent); overflow: hidden; box-shadow: 0 10px 26px rgba(0, 0, 0, .06); }
.gm-header, .gm-status, .gm-result { display: flex; gap: 8px; align-items: flex-start; padding: 12px 14px; font-size: 13px; color: var(--color-text-2); }
.gm-header { align-items: center; font-weight: 650; color: #a78bfa; border-bottom: 1px solid var(--color-border-2); background: rgba(167, 139, 250, .07); }
.gm-list { padding: 8px 14px; max-height: 150px; overflow-y: auto; }
.gm-item, .gm-more { font-size: 12px; color: var(--color-text-3); padding: 2px 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gm-actions { display: flex; gap: 8px; padding: 10px 14px; border-top: 1px solid var(--color-border-2); }
.gm-btn { padding: 6px 13px; font-size: 12px; border-radius: 8px; cursor: pointer; font-family: inherit; border: 1px solid var(--color-border-2); background: var(--color-bg-2); color: var(--color-text-1); }
.gm-btn-primary { color: #fff; background: rgb(var(--primary-6)); border-color: transparent; }
.gm-result { color: rgb(var(--success-6)); }
.gm-result pre { margin: 0; white-space: pre-wrap; word-break: break-word; color: var(--color-text-2); }
.gm-error { color: rgb(var(--danger-6)); }
.gm-spin { animation: gm-spin 1s linear infinite; }
@keyframes gm-spin { to { transform: rotate(360deg); } }
</style>
