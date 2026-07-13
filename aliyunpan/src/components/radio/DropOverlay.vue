<script setup lang="ts">
/**
 * DropOverlay — file drag-and-drop handler for music and covers.
 * Shows a fullscreen glass panel when files are dragged over the window.
 * Emits file lists for the parent to handle.
 */
import { ref, onMounted, onBeforeUnmount } from 'vue'

const emit = defineEmits<{
  (e: 'files-dropped', files: File[]): void
}>()

const dragging = ref(false)
let dragCounter = 0

function handleDragEnter(e: DragEvent) {
  e.preventDefault()
  dragCounter++
  if (dragCounter === 1) dragging.value = true
}

function handleDragLeave(e: DragEvent) {
  e.preventDefault()
  dragCounter--
  if (dragCounter <= 0) {
    dragCounter = 0
    dragging.value = false
  }
}

function handleDragOver(e: DragEvent) {
  e.preventDefault()
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy'
}

function handleDrop(e: DragEvent) {
  e.preventDefault()
  dragging.value = false
  dragCounter = 0
  const files = e.dataTransfer?.files
  if (files && files.length) {
    emit('files-dropped', Array.from(files))
  }
}

onMounted(() => {
  document.addEventListener('dragenter', handleDragEnter)
  document.addEventListener('dragleave', handleDragLeave)
  document.addEventListener('dragover', handleDragOver)
  document.addEventListener('drop', handleDrop)
})

onBeforeUnmount(() => {
  document.removeEventListener('dragenter', handleDragEnter)
  document.removeEventListener('dragleave', handleDragLeave)
  document.removeEventListener('dragover', handleDragOver)
  document.removeEventListener('drop', handleDrop)
})
</script>

<template>
  <div v-if="dragging" class="drop-overlay">
    <div class="drop-panel">
      <div class="drop-icon">↓</div>
      <div class="drop-title">拖放音乐或封面</div>
      <div class="drop-sub">支持 MP3 / FLAC / WAV / OGG / M4A / JPG / PNG / WebP</div>
    </div>
  </div>
</template>

<style scoped lang="less">
.drop-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  background: rgba(0, 0, 0, 0.62);
  backdrop-filter: blur(12px) saturate(1.2);
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: all;
}
.drop-panel {
  padding: 48px 64px;
  border-radius: 24px;
  border: 2px dashed rgba(0, 245, 212, 0.32);
  background: linear-gradient(145deg, rgba(14, 16, 20, 0.88), rgba(5, 6, 8, 0.94));
  box-shadow: 0 32px 96px rgba(0, 0, 0, 0.5), inset 0 1px 0 rgba(255, 255, 255, 0.06);
  text-align: center;
  color: #fff;
}
.drop-icon { font-size: 48px; color: rgba(0, 245, 212, 0.8); margin-bottom: 16px; }
.drop-title { font-size: 18px; font-weight: 700; margin-bottom: 8px; }
.drop-sub { font-size: 12px; color: rgba(255, 255, 255, 0.45); }
</style>
