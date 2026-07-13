<script setup lang="ts">
import { AlertCircle, CheckCircle, Loader2, UploadCloud } from 'lucide-vue-next'

defineProps<{
  state: 'parsing' | 'confirm' | 'running' | 'done' | 'error'
  input?: { parentId?: string; files?: { path: string; name: string; size: number }[] }
  output?: { total: number; success?: number; failed?: number; skipped?: number; report?: string }
  error?: string
}>()

const emit = defineEmits<{ (e: 'confirm'): void; (e: 'cancel'): void }>()
</script>

<template>
  <div class="mc-card">
    <div v-if="state === 'parsing' || state === 'running'" class="mc-status">
      <Loader2 :size="14" :stroke-width="2" class="mc-spin" />
      <span>{{ state === 'parsing' ? '正在解析秒传清单...' : '正在导入光鸭云盘...' }}</span>
    </div>

    <template v-else-if="state === 'confirm'">
      <div class="mc-header">
        <UploadCloud :size="16" :stroke-width="1.5" />
        <span>确认导入 {{ input?.files?.length || 0 }} 个秒传文件到光鸭云盘？</span>
      </div>
      <div class="mc-list">
        <div v-for="(file, index) in (input?.files || []).slice(0, 10)" :key="index" class="mc-file">{{ file.path }}</div>
        <div v-if="(input?.files?.length || 0) > 10" class="mc-more">…还有 {{ (input?.files?.length || 0) - 10 }} 个文件</div>
      </div>
      <div class="mc-actions">
        <button class="mc-btn mc-btn-primary" @click="emit('confirm')">确认导入</button>
        <button class="mc-btn" @click="emit('cancel')">取消</button>
      </div>
    </template>

    <div v-else-if="state === 'done'" class="mc-result">
      <CheckCircle :size="16" :stroke-width="1.5" />
      <div>
        <div>秒传清单处理完成：{{ output?.success ?? output?.total ?? 0 }}/{{ output?.total ?? 0 }}</div>
        <pre v-if="output?.report" class="mc-report">{{ output.report }}</pre>
      </div>
    </div>

    <div v-else class="mc-status mc-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>{{ error || '秒传工具执行失败' }}</span>
    </div>
  </div>
</template>

<style scoped>
.mc-card { margin: 8px 0; border-radius: 8px; background: var(--color-fill-1); border: 1px solid var(--color-border-2); overflow: hidden; }
.mc-header, .mc-status, .mc-result { display: flex; gap: 8px; align-items: flex-start; padding: 12px 14px; font-size: 13px; color: var(--color-text-2); }
.mc-header { align-items: center; font-weight: 500; color: var(--color-text-1); border-bottom: 1px solid var(--color-border-2); }
.mc-list { padding: 8px 14px; max-height: 160px; overflow-y: auto; }
.mc-file, .mc-more { font-size: 12px; color: var(--color-text-3); padding: 2px 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mc-more { color: var(--color-text-4); }
.mc-actions { display: flex; gap: 8px; padding: 10px 14px; border-top: 1px solid var(--color-border-2); }
.mc-btn { padding: 4px 14px; font-size: 12px; border-radius: 6px; cursor: pointer; font-family: inherit; border: 1px solid var(--color-border-2); background: var(--color-bg-2); color: var(--color-text-1); }
.mc-btn-primary { color: #fff; background: rgb(var(--primary-6)); border-color: transparent; }
.mc-result { color: rgb(var(--success-6)); }
.mc-error { color: rgb(var(--danger-6)); }
.mc-report { margin: 6px 0 0; white-space: pre-wrap; word-break: break-word; color: var(--color-text-3); font-size: 12px; line-height: 1.6; }
.mc-spin { animation: mc-spin 1s linear infinite; }
@keyframes mc-spin { to { transform: rotate(360deg); } }
</style>
