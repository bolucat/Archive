<script setup lang="ts">
import { AlertCircle, CheckCircle, FolderSearch, Loader2 } from 'lucide-vue-next'

defineProps<{
  state: 'scanning' | 'confirm' | 'running' | 'done' | 'error'
  input?: { dirs?: { path: string }[] }
  output?: { scannedDirs?: number; total: number; success?: number; failed?: number; report: string }
  error?: string
}>()

const emit = defineEmits<{ (e: 'confirm'): void; (e: 'cancel'): void }>()
</script>

<template>
  <div class="ed-card">
    <div v-if="state === 'scanning' || state === 'running'" class="ed-status">
      <Loader2 :size="14" :stroke-width="2" class="ed-spin" />
      <span>{{ state === 'scanning' ? '正在扫描网盘空目录...' : '正在删除空目录...' }}</span>
    </div>

    <template v-else-if="state === 'confirm'">
      <div class="ed-header">
        <FolderSearch :size="16" :stroke-width="1.5" />
        <span>确认删除 {{ input?.dirs?.length || 0 }} 个空目录？</span>
      </div>
      <div class="ed-list">
        <div v-for="(dir, index) in (input?.dirs || []).slice(0, 10)" :key="index" class="ed-item">{{ dir.path }}</div>
        <div v-if="(input?.dirs?.length || 0) > 10" class="ed-more">…还有 {{ (input?.dirs?.length || 0) - 10 }} 个</div>
      </div>
      <div class="ed-actions">
        <button class="ed-btn ed-btn-danger" @click="emit('confirm')">确认删除</button>
        <button class="ed-btn" @click="emit('cancel')">取消</button>
      </div>
    </template>

    <div v-else-if="state === 'done'" class="ed-result">
      <CheckCircle :size="16" :stroke-width="1.5" />
      <pre>{{ output?.report }}</pre>
    </div>

    <div v-else class="ed-status ed-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>{{ error || '空目录操作失败' }}</span>
    </div>
  </div>
</template>

<style scoped>
.ed-card { margin: 12px 0; border-radius: 13px; background: color-mix(in srgb, var(--color-fill-1) 78%, transparent); border: 1px solid color-mix(in srgb, var(--color-border-2) 88%, transparent); overflow: hidden; box-shadow: 0 10px 26px rgba(0, 0, 0, .06); }
.ed-header, .ed-status, .ed-result { display: flex; gap: 8px; align-items: flex-start; padding: 12px 14px; font-size: 13px; color: var(--color-text-2); }
.ed-header { align-items: center; font-weight: 650; color: #f4b84f; border-bottom: 1px solid var(--color-border-2); background: rgba(244, 184, 79, .07); }
.ed-list { padding: 8px 14px; max-height: 150px; overflow-y: auto; }
.ed-item, .ed-more { font-size: 12px; color: var(--color-text-3); padding: 2px 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ed-actions { display: flex; gap: 8px; padding: 10px 14px; border-top: 1px solid var(--color-border-2); }
.ed-btn { padding: 6px 13px; font-size: 12px; border-radius: 8px; cursor: pointer; font-family: inherit; border: 1px solid var(--color-border-2); background: var(--color-bg-2); color: var(--color-text-1); }
.ed-btn-danger { color: #fff; background: rgb(var(--danger-6)); border-color: transparent; }
.ed-result { color: rgb(var(--success-6)); }
.ed-result pre { margin: 0; white-space: pre-wrap; word-break: break-word; color: var(--color-text-2); }
.ed-error { color: rgb(var(--danger-6)); }
.ed-spin { animation: ed-spin 1s linear infinite; }
@keyframes ed-spin { to { transform: rotate(360deg); } }
</style>
