<script setup lang='ts'>
import { ref, onMounted, onUnmounted } from 'vue'
import IconFont from '../../components/IconFont.vue'

const props = defineProps<{
  bookHash: string
  currentPage: number
}>()

const emit = defineEmits<{
  (e: 'index'): void
  (e: 'reindex'): void
  (e: 'abort'): void
}>()

const status = ref<'idle' | 'indexing' | 'done' | 'error'>('idle')
const progressText = ref('未索引')
const progressPercent = ref(0)
const phase = ref<'chunking' | 'embedding'>('chunking')
const chunkCount = ref(0)
const errorMessage = ref('')

function updateProgress(data: { phase?: string; current: number; total: number; status?: string }) {
  if (data.status) status.value = data.status as any
  if (data.phase) phase.value = data.phase as any
  if (data.total > 0) {
    progressPercent.value = Math.round((data.current / data.total) * 100)
  }
  if (data.phase === 'chunking') {
    progressText.value = `正在分块... ${data.current}/${data.total} 节`
  } else if (data.phase === 'embedding') {
    progressText.value = `正在嵌入... ${data.current}/${data.total} 块`
  }
  if (data.status === 'done') {
    status.value = 'done'
    progressText.value = `已索引 ${chunkCount.value} 个块`
  } else if (data.status === 'error') {
    status.value = 'error'
    progressText.value = `索引失败: ${errorMessage.value}`
  }
  status.value = (data.status || 'indexing') as 'idle' | 'indexing' | 'done' | 'error'
}
</script>

<template>
  <div class="reedy-indexing-status" v-if="status !== 'done'">
    <div class="status-row">
      <IconFont name="iconsearch" :spin="status === 'indexing'" />
      <span class="status-text">{{ progressText }} {{ status === 'indexing' ? `${progressPercent}%` : '' }}</span>
      <a-button
        v-if="status === 'idle'"
        size="mini"
        type="primary"
        @click="emit('index')"
      >开始索引</a-button>
      <a-button
        v-else-if="status === 'error'"
        size="mini"
        type="primary"
        @click="emit('reindex')"
      >重新索引</a-button>
      <a-button
        v-else-if="status === 'indexing'"
        size="mini"
        @click="emit('abort')"
      >取消</a-button>
    </div>
    <div v-if="status === 'indexing'" class="progress-bar">
      <div class="progress-fill" :style="{ width: progressPercent + '%' }" />
    </div>
  </div>
</template>

<style scoped>
.reedy-indexing-status {
  padding: 6px 8px;
  background: var(--color-bg-3);
  border-radius: 4px;
  margin-bottom: 8px;
}
.status-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.status-text {
  flex: 1;
  font-size: 12px;
  color: var(--color-text-2);
}
.progress-bar {
  height: 3px;
  background: var(--color-border-2);
  border-radius: 2px;
  margin-top: 4px;
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  background: rgb(var(--primary-6));
  transition: width 0.3s ease;
}
</style>
