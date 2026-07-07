<script setup lang='ts'>
import { ref } from 'vue'

const emit = defineEmits<{
  (e: 'drop-url', url: string): void
  (e: 'drop-file', filePath: string, base64: string): void
}>()

const isDragging = ref(false)
let dragCounter = 0

const onDragEnter = (e: DragEvent) => {
  e.preventDefault()
  dragCounter++
  isDragging.value = true
}

const onDragLeave = (e: DragEvent) => {
  e.preventDefault()
  dragCounter--
  if (dragCounter <= 0) {
    dragCounter = 0
    isDragging.value = false
  }
}

const onDragOver = (e: DragEvent) => {
  e.preventDefault()
}

const onDrop = async (e: DragEvent) => {
  e.preventDefault()
  dragCounter = 0
  isDragging.value = false

  const dt = e.dataTransfer
  if (!dt) return

  const url = dt.getData('text/uri-list') || dt.getData('text/plain') || ''
  if (url && (url.startsWith('http') || url.startsWith('magnet'))) {
    emit('drop-url', url.trim())
    return
  }

  const files = dt.files
  if (files && files.length > 0) {
    const file = files[0]
    if (file.name.toLowerCase().endsWith('.torrent')) {
      const reader = new FileReader()
      reader.onload = () => {
        const base64 = (reader.result as string).split(',')[1] || ''
        emit('drop-file', file.name, base64)
      }
      reader.readAsDataURL(file)
    }
  }
}
</script>

<template>
  <div
    class='drag-drop-zone'
    :class="{ 'drag-active': isDragging }"
    @dragenter='onDragEnter'
    @dragleave='onDragLeave'
    @dragover='onDragOver'
    @drop='onDrop'
  >
    <slot />
    <div v-if='isDragging' class='drag-overlay'>
      <div class='drag-hint'>
        <IconFont name='iconcloud-download' />
        <span>松开以添加下载任务</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.drag-drop-zone {
  position: relative;
  width: 100%;
  height: 100%;
}
.drag-active {
  outline: 2px dashed var(--color-primary-6, #637dff);
  outline-offset: -2px;
  border-radius: 4px;
}
.drag-overlay {
  position: absolute;
  inset: 0;
  background: rgba(99, 125, 255, 0.08);
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  z-index: 10;
  border-radius: 4px;
}
.drag-hint {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  color: var(--color-primary-6, #637dff);
  font-size: 14px;
  font-weight: 500;
}
.drag-hint .iconfont {
  font-size: 32px;
}
</style>
