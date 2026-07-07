<script setup lang='ts'>
defineProps<{
  cfi: string
  chapter: string
  text: string
}>()

defineEmits<{
  (e: 'click', cfi: string): void
}>()
</script>

<template>
  <div class="citation-card" @click="$emit('click', cfi)" :title="text">
    <span class="citation-icon">"</span>
    <span class="citation-chapter">{{ chapter }}</span>
    <span class="citation-text">{{ text.slice(0, 80) }}{{ text.length > 80 ? '...' : '' }}</span>
  </div>
</template>

<style scoped>
.citation-card {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 6px 8px;
  margin: 4px 0;
  background: var(--color-bg-3);
  border-radius: 4px;
  border-left: 3px solid rgb(var(--primary-6));
  cursor: pointer;
  font-size: 12px;
  transition: background 0.2s;
}
.citation-card:hover {
  background: var(--color-bg-4);
}
.citation-icon {
  color: rgb(var(--primary-6));
  font-size: 16px;
  font-weight: bold;
  line-height: 1;
  margin-top: 2px;
}
.citation-chapter {
  color: rgb(var(--primary-6));
  font-weight: 600;
  flex-shrink: 0;
}
.citation-text {
  color: var(--color-text-2);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
