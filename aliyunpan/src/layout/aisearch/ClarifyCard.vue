<script setup lang="ts">
import { HelpCircle } from 'lucide-vue-next'

defineProps<{ question: string; options: string[] }>()
const emit = defineEmits<{ (e: 'select', option: string): void }>()
</script>

<template>
  <div class="clarify-card">
    <div class="clarify-header">
      <HelpCircle :size="16" :stroke-width="1.5" />
      <span>{{ question }}</span>
    </div>
    <div class="clarify-options">
      <button
        v-for="(opt, i) in options"
        :key="i"
        class="clarify-option"
        @click="emit('select', opt)"
      >
        {{ opt }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.clarify-card { margin: 8px 0; padding: 14px; border-radius: 10px; background: rgba(var(--primary-6), 0.05); border: 1px solid rgba(var(--primary-6), 0.18); }
.clarify-header { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; font-size: 14px; font-weight: 500; color: var(--color-text-1); }
.clarify-options { display: flex; flex-wrap: wrap; gap: 8px; }
.clarify-option { padding: 6px 14px; font-size: 13px; color: rgb(var(--primary-6)); background: transparent; border: 1px solid rgba(var(--primary-6), 0.3); border-radius: 16px; cursor: pointer; transition: all 0.15s; font-family: inherit; }
.clarify-option:hover { background: rgba(var(--primary-6), 0.1); border-color: rgb(var(--primary-6)); }
</style>
