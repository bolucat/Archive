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
.clarify-card { margin: 12px 0; padding: 15px; border-radius: 13px; background: linear-gradient(135deg, rgba(var(--primary-6), .11), rgba(139, 92, 246, .06)); border: 1px solid rgba(var(--primary-6), .25); box-shadow: 0 10px 26px rgba(0, 0, 0, .05); }
.clarify-header { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; font-size: 14px; font-weight: 500; color: var(--color-text-1); }
.clarify-options { display: flex; flex-wrap: wrap; gap: 8px; }
.clarify-option { padding: 6px 12px; font-size: 12px; color: rgb(var(--primary-6)); background: rgba(var(--primary-6), .04); border: 1px solid rgba(var(--primary-6), .3); border-radius: 8px; cursor: pointer; transition: all .15s; font-family: inherit; }
.clarify-option:hover { background: rgba(var(--primary-6), 0.1); border-color: rgb(var(--primary-6)); }
</style>
