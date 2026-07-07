<script setup lang="ts">
import { ref } from 'vue'
import { ChevronRight, Lightbulb } from 'lucide-vue-next'

defineProps<{ text: string }>()
const open = ref(true)
</script>

<template>
  <div class="reason-chain">
    <button class="reason-header" @click="open = !open">
      <ChevronRight :size="12" :stroke-width="2" class="reason-chevron" :class="{ open }" />
      <Lightbulb :size="14" :stroke-width="1.5" />
      <span>思考过程</span>
    </button>
    <div v-show="open" class="reason-body">{{ text }}</div>
  </div>
</template>

<style scoped>
.reason-chain { margin: 4px 0; border-radius: 8px; background: rgba(var(--warning-6), 0.06); border: 1px solid rgba(var(--warning-6), 0.15); overflow: hidden; }
.reason-header { display: flex; align-items: center; gap: 6px; width: 100%; padding: 8px 12px; border: 0; background: transparent; color: var(--color-text-3); font-size: 12px; font-weight: 600; cursor: pointer; font-family: inherit; }
.reason-header:hover { background: rgba(var(--warning-6), 0.06); }
.reason-chevron { flex-shrink: 0; transition: transform 0.15s; }
.reason-chevron.open { transform: rotate(90deg); }
.reason-body { padding: 8px 12px 12px 32px; font-size: 13px; color: var(--color-text-3); line-height: 1.6; white-space: pre-wrap; word-break: break-word; }
</style>
