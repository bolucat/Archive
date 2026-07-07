<script setup lang="ts">
import type { CSSProperties } from 'vue'

type ReaderEdgeSide = 'top' | 'left' | 'right' | 'bottom'

defineProps<{
  openPanels: Record<ReaderEdgeSide, boolean>
  lockedPanels: Record<ReaderEdgeSide, boolean>
  shellStyle?: CSSProperties
}>()

const emit = defineEmits<{
  (e: 'enter', side: ReaderEdgeSide): void
  (e: 'leave', side: ReaderEdgeSide): void
}>()
</script>

<template>
  <section class="reader-shell" :style="shellStyle">
    <slot />
  </section>
</template>

<style scoped>
.reader-shell {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--reader-page, var(--color-bg-1));
  color: var(--reader-text, var(--color-text-1));
}
</style>
