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
.summary-text :deep(h1), .summary-text :deep(h2), .summary-text :deep(h3), .summary-text :deep(h4) { margin: 8px 0 4px; font-weight: 600; line-height: 1.3; }
.summary-text :deep(h1) { font-size: 18px; }
.summary-text :deep(h2) { font-size: 16px; }
.summary-text :deep(h3) { font-size: 15px; }
.summary-text :deep(ul), .summary-text :deep(ol) { padding-left: 20px; margin: 4px 0; }
.summary-text :deep(li) { margin: 2px 0; }
.summary-text :deep(table) { width: 100%; border-collapse: collapse; margin: 8px 0; font-size: 13px; }
.summary-text :deep(th), .summary-text :deep(td) { padding: 6px 10px; border: 1px solid var(--color-border-2); text-align: left; }
.summary-text :deep(th) { background: var(--color-fill-1); font-weight: 600; }
.summary-text :deep(pre) { margin: 6px 0; padding: 10px 12px; background: var(--color-fill-1); border-radius: 6px; overflow-x: auto; font-size: 12px; line-height: 1.5; }
.summary-text :deep(pre code) { padding: 0; background: transparent; font-size: inherit; }
.summary-text :deep(blockquote) { margin: 6px 0; padding: 4px 12px; border-left: 3px solid rgb(var(--primary-6)); color: var(--color-text-2); background: var(--color-fill-1); border-radius: 0 4px 4px 0; }
.summary-text :deep(hr) { border: 0; border-top: 1px solid var(--color-border-2); margin: 10px 0; }
.summary-followups { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; margin-top: 10px; padding-top: 10px; border-top: 1px solid var(--color-border-2); }
.summary-followups-label { font-size: 12px; color: var(--color-text-4); }
.summary-followup-btn { padding: 4px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: rgba(var(--primary-6), 0.06); border: 1px solid rgba(var(--primary-6), 0.2); border-radius: 12px; cursor: pointer; transition: all 0.15s; font-family: inherit; }
.summary-followup-btn:hover { background: rgba(var(--primary-6), 0.12); }
</style>
