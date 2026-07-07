<script setup lang="ts">
defineProps<{ suggestions: string[] }>()
const emit = defineEmits<{ (e: 'select', query: string): void }>()
</script>

<template>
  <div v-if="suggestions.length" class="followup-bar">
    <span class="followup-label">试试问:</span>
    <button
      v-for="(s, i) in suggestions"
      :key="i"
      class="followup-chip"
      @click="emit('select', s)"
    >
      {{ s }}
    </button>
  </div>
</template>

<style scoped>
.followup-bar { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; padding: 8px 48px; border-top: 1px solid var(--color-border-2); background: var(--color-bg-1); }
.followup-label { font-size: 12px; color: var(--color-text-4); flex-shrink: 0; }
.followup-chip { padding: 3px 10px; font-size: 12px; color: var(--color-text-3); background: var(--color-fill-1); border: 1px solid var(--color-border-2); border-radius: 12px; cursor: pointer; font-family: inherit; transition: all 0.15s; }
.followup-chip:hover { color: rgb(var(--primary-6)); border-color: rgb(var(--primary-6)); }
</style>
