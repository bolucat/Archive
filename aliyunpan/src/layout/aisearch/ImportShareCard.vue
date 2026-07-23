<script setup lang="ts">
import { Loader2, CheckCircle, AlertCircle } from 'lucide-vue-next'
import { t } from '../../i18n'

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
    <div v-if="state === 'parsing'" class="is-status"><Loader2 :size="14" :stroke-width="2" class="is-spin" /> {{ t('ai.card.parsingShare') }}</div>
    <div v-else-if="state === 'listing'" class="is-status"><Loader2 :size="14" :stroke-width="2" class="is-spin" /> {{ t('ai.card.listingShareFiles') }}</div>
    <div v-else-if="state === 'saving'" class="is-status"><Loader2 :size="14" :stroke-width="2" class="is-spin" /> {{ t('ai.card.savingToDrive') }}</div>
    <div v-else-if="state === 'done' && output && output.asyncStatus" class="is-status is-done">
      <CheckCircle :size="16" :stroke-width="1.5" />
      <span>{{ t('ai.card.shareImportSubmittedPrefix') }} {{ output.platform }} {{ t('ai.card.share') }}「{{ output.shareName }}」{{ t('ai.card.submittedSave') }} {{ output.fileCount }} {{ t('transfer.file') }}，{{ t('ai.card.processingInBackground') }}</span>
    </div>
    <div v-else-if="state === 'done' && output" class="is-status is-done">
      <CheckCircle :size="16" :stroke-width="1.5" />
      <span>{{ t('ai.card.shareImportSubmittedPrefix') }} {{ output.platform }} {{ t('ai.card.share') }}「{{ output.shareName }}」{{ t('ai.card.saved') }} {{ output.savedCount }}/{{ output.fileCount }} {{ t('transfer.file') }}</span>
    </div>
    <div v-else-if="state === 'error'" class="is-status is-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>{{ error || t('ai.card.importFailed') }}</span>
      <button class="is-retry" @click="emit('retry')">{{ t('common.retry') }}</button>
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
