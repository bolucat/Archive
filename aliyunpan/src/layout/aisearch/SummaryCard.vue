<script setup lang="ts">
import { renderMarkdown } from './markdown'

const props = defineProps<{ text: string; followups: string[] }>()
const emit = defineEmits<{ (e: 'followup', query: string): void }>()
</script>

<template>
  <div v-if="text || followups.length" class="summary-card">
    <div v-if="text" class="summary-text" v-html="renderMarkdown(text)" />
    <div v-if="followups.length" class="summary-followups">
      <span class="summary-followups-label">继续搜索:</span>
      <button
        v-for="(q, i) in followups"
        :key="i"
        class="summary-followup-btn"
        @click="emit('followup', q)"
      >
        {{ q }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.summary-card { margin: 12px 0; padding: 12px 16px; border-radius: 8px; background: var(--color-fill-1); border: 1px solid var(--color-border-2); }
.summary-text { font-size: 14px; color: var(--color-text-1); line-height: 1.65; }
.summary-text :deep(strong) { color: var(--color-text-1); font-weight: 600; }
.summary-text :deep(code) { padding: 1px 4px; font-size: 12px; background: var(--color-fill-2); border-radius: 3px; }
.summary-text :deep(a) { color: rgb(var(--primary-6)); }
.summary-followups { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; margin-top: 10px; padding-top: 10px; border-top: 1px solid var(--color-border-2); }
.summary-followups-label { font-size: 12px; color: var(--color-text-4); }
.summary-followup-btn { padding: 4px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: rgba(var(--primary-6), 0.06); border: 1px solid rgba(var(--primary-6), 0.2); border-radius: 12px; cursor: pointer; transition: all 0.15s; font-family: inherit; }
.summary-followup-btn:hover { background: rgba(var(--primary-6), 0.12); }
</style>
