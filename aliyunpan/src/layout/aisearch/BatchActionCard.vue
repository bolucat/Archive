<script setup lang="ts">
import { AlertCircle, Loader2, CheckCircle } from 'lucide-vue-next'
import { t } from '../../i18n'

defineProps<{
  action: 'delete' | 'move'
  state: 'confirm' | 'running' | 'done' | 'error'
  files: { name: string; fileId: string; driveId: string; userId: string }[]
  targetDir?: string
  output?: { total: number; success: number; failed: number; report?: string }
  error?: string
}>()

const emit = defineEmits<{ (e: 'confirm'): void; (e: 'cancel'): void; (e: 'retry'): void }>()
</script>

<template>
  <div class="ba-card">
    <template v-if="state === 'confirm'">
      <div class="ba-header">
        <AlertCircle :size="16" :stroke-width="1.5" />
        <span>{{ action === 'delete' ? t('ai.card.confirmTrash') : t('ai.card.confirmMove') }} {{ files.length }} {{ t('transfer.file') }}{{ action === 'move' && targetDir ? `${t('ai.card.to')} ${targetDir}` : '' }}?</span>
      </div>
      <div class="ba-file-list">
        <div v-for="(f, i) in files.slice(0, 10)" :key="i" class="ba-file-name">{{ f.name }}</div>
        <div v-if="files.length > 10" class="ba-more">…{{ t('ai.card.more') }} {{ files.length - 10 }} {{ t('transfer.file') }}</div>
      </div>
      <div class="ba-actions">
        <button class="ba-btn ba-btn-danger" @click="emit('confirm')">
          {{ action === 'delete' ? t('ai.card.confirmTrash') : t('ai.card.confirmMove') }}
        </button>
        <button class="ba-btn ba-btn-cancel" @click="emit('cancel')">{{ t('common.cancel') }}</button>
      </div>
    </template>

    <div v-else-if="state === 'running'" class="ba-status">
      <Loader2 :size="14" :stroke-width="2" class="ba-spin" />
      <span>{{ action === 'delete' ? t('ai.card.trashing') : t('ai.card.moving') }}...</span>
    </div>

    <div v-else-if="state === 'done'" class="ba-status ba-done">
      <CheckCircle :size="16" :stroke-width="1.5" />
      <span>{{ action === 'delete' ? t('ai.card.trashed') : t('ai.card.moved') }} {{ output?.success || 0 }}/{{ output?.total || 0 }}{{ output?.failed ? `，${output.failed} ${t('footer.failed')}` : '' }}{{ output?.report ? `。${output.report}` : '' }}</span>
    </div>

    <div v-else-if="state === 'error'" class="ba-status ba-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>{{ error || t('ai.card.operationFailed') }}</span>
      <button class="ba-btn ba-btn-retry" @click="emit('retry')">{{ t('common.retry') }}</button>
    </div>
  </div>
</template>

<style scoped>
.ba-card { margin: 12px 0; border-radius: 13px; background: color-mix(in srgb, var(--color-fill-1) 78%, transparent); border: 1px solid color-mix(in srgb, var(--color-border-2) 88%, transparent); overflow: hidden; box-shadow: 0 10px 26px rgba(0, 0, 0, .06); }
.ba-header { display: flex; align-items: center; gap: 8px; padding: 13px 14px; font-size: 13px; font-weight: 650; color: var(--color-text-1); border-bottom: 1px solid var(--color-border-2); background: rgba(245, 158, 11, .07); }
.ba-file-list { padding: 8px 14px; max-height: 160px; overflow-y: auto; }
.ba-file-name { font-size: 12px; color: var(--color-text-3); padding: 2px 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ba-more { font-size: 12px; color: var(--color-text-4); padding: 4px 0; }
.ba-actions { display: flex; gap: 8px; padding: 10px 14px; border-top: 1px solid var(--color-border-2); }
.ba-btn { padding: 6px 13px; font-size: 12px; border-radius: 8px; cursor: pointer; font-family: inherit; border: 1px solid var(--color-border-2); background: var(--color-bg-2); color: var(--color-text-1); }
.ba-btn-danger { background: rgb(var(--danger-6)); color: #fff; border-color: transparent; }
.ba-btn-danger:hover { opacity: 0.9; }
.ba-btn-cancel:hover { background: var(--color-fill-2); }
.ba-btn-retry { margin-left: auto; padding: 2px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: transparent; border: 1px solid rgb(var(--primary-6)); border-radius: 4px; cursor: pointer; }
.ba-status { display: flex; align-items: center; gap: 8px; padding: 12px 14px; font-size: 13px; color: var(--color-text-3); }
.ba-done { color: rgb(var(--success-6)); }
.ba-error { color: rgb(var(--danger-6)); }
.ba-spin { animation: ba-spin 1s linear infinite; }
@keyframes ba-spin { to { transform: rotate(360deg); } }
</style>
