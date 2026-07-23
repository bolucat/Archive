<script setup lang="ts">
import { ref } from 'vue'
import { ChevronRight, ListChecks } from 'lucide-vue-next'
import { t } from '../../i18n'

defineProps<{ text: string }>()
const open = ref(true)
</script>

<template>
  <div class="reason-chain">
    <button class="reason-header" @click="open = !open">
      <ChevronRight :size="12" :stroke-width="2" class="reason-chevron" :class="{ open }" />
      <ListChecks :size="14" :stroke-width="1.5" />
      <span>{{ t('ai.card.executionNotes') }}</span>
    </button>
    <div v-show="open" class="reason-body">{{ text }}</div>
  </div>
</template>

<style scoped>
.reason-chain { margin: 10px 0; border-radius: 11px; background: rgba(var(--warning-6), .055); border: 1px solid rgba(var(--warning-6), .20); overflow: hidden; }
.reason-header { display: flex; align-items: center; gap: 7px; width: 100%; padding: 9px 12px; border: 0; background: transparent; color: #dca84b; font-size: 11px; font-weight: 700; cursor: pointer; font-family: inherit; letter-spacing: .03em; }
.reason-header:hover { background: rgba(var(--warning-6), 0.06); }
.reason-chevron { flex-shrink: 0; transition: transform 0.15s; }
.reason-chevron.open { transform: rotate(90deg); }
.reason-body { padding: 8px 12px 12px 32px; font-size: 13px; color: var(--color-text-3); line-height: 1.6; white-space: pre-wrap; word-break: break-word; }
</style>
