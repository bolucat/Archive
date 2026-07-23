<script setup lang="ts">
import { FolderTree, Loader2, AlertCircle } from 'lucide-vue-next'
import { humanSize } from '../../utils/format'
import { t } from '../../i18n'

defineProps<{
  state: 'planning' | 'done' | 'error'
  output?: { categories: { name: string; pattern: string; fileCount: number; totalSize: number }[] }
  error?: string
}>()

const emit = defineEmits<{ (e: 'organize', categories: { name: string; pattern: string; fileCount: number; totalSize: number }[]): void; (e: 'retry'): void }>()
</script>

<template>
  <div class="oc-card">
    <div v-if="state === 'planning'" class="oc-status">
      <Loader2 :size="14" :stroke-width="2" class="oc-spin" />
      <span>{{ t('ai.card.analyzingCategories') }}</span>
    </div>

    <template v-else-if="state === 'done' && output">
      <div class="oc-header"><FolderTree :size="14" :stroke-width="1.5" /> {{ t('ai.card.categorySuggestions') }}</div>
      <div class="oc-table">
        <div class="oc-row oc-row-head">
          <span>{{ t('ai.card.category') }}</span><span>{{ t('ai.card.match') }}</span><span>{{ t('ai.card.fileCount') }}</span><span>{{ t('transfer.size') }}</span>
        </div>
        <div v-for="c in output.categories" :key="c.name" class="oc-row">
          <span class="oc-name">{{ c.name }}</span>
          <span class="oc-pattern">{{ c.pattern }}</span>
          <span>{{ c.fileCount }}</span>
          <span>{{ humanSize(c.totalSize) }}</span>
        </div>
      </div>
      <div class="oc-action">
        <button class="oc-btn" @click="emit('organize', output.categories)">{{ t('ai.card.organizeWithPlan') }}</button>
      </div>
    </template>

    <div v-else-if="state === 'error'" class="oc-status oc-error">
      <AlertCircle :size="14" :stroke-width="1.5" /><span>{{ error }}</span>
      <button class="oc-retry" @click="emit('retry')">{{ t('common.retry') }}</button>
    </div>
  </div>
</template>

<style scoped>
.oc-card { margin: 12px 0; border-radius: 13px; background: color-mix(in srgb, var(--color-fill-1) 78%, transparent); border: 1px solid color-mix(in srgb, var(--color-border-2) 88%, transparent); overflow: hidden; box-shadow: 0 10px 26px rgba(0, 0, 0, .06); }
.oc-status { display: flex; align-items: center; gap: 8px; padding: 12px 14px; font-size: 13px; color: var(--color-text-3); }
.oc-error { color: rgb(var(--danger-6)); }
.oc-header { display: flex; align-items: center; gap: 7px; padding: 11px 14px; font-size: 12px; font-weight: 650; color: #a78bfa; border-bottom: 1px solid var(--color-border-2); background: rgba(167, 139, 250, .07); }
.oc-table { padding: 4px 0; }
.oc-row { display: grid; grid-template-columns: 2fr 1fr 0.8fr 1fr; gap: 8px; padding: 6px 14px; font-size: 12px; color: var(--color-text-2); }
.oc-row-head { color: var(--color-text-4); font-weight: 600; border-bottom: 1px solid var(--color-border-2); }
.oc-name { font-weight: 500; }
.oc-pattern { color: var(--color-text-4); font-family: monospace; font-size: 11px; }
.oc-action { padding: 10px 14px; border-top: 1px solid var(--color-border-2); }
.oc-btn { padding: 6px 13px; font-size: 12px; color: #fff; background: linear-gradient(110deg, #8b5cf6, rgb(var(--primary-6))); border: 0; border-radius: 8px; cursor: pointer; font-family: inherit; }
.oc-btn:hover { opacity: 0.9; }
.oc-retry { margin-left: auto; padding: 2px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: transparent; border: 1px solid rgb(var(--primary-6)); border-radius: 4px; cursor: pointer; }
.oc-spin { animation: oc-spin 1s linear infinite; }
@keyframes oc-spin { to { transform: rotate(360deg); } }
</style>
