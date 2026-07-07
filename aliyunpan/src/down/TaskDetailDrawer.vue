<script setup lang='ts'>
import { ref, watch, computed } from 'vue'
import { getTaskStatus, getTaskFiles } from './integration/aria2TaskApi'
import type { DownloadTask, DownloadTaskFile } from './integration/taskTypes'

const props = defineProps<{
  visible: boolean
  gid: string
}>()
const emit = defineEmits<{
  (e: 'update:visible', val: boolean): void
}>()

const task = ref<DownloadTask | null>(null)
const files = ref<DownloadTaskFile[]>([])
const loading = ref(false)

watch(
  () => props.visible,
  async (v) => {
    if (!v || !props.gid) return
    loading.value = true
    try {
      const [t, f] = await Promise.all([
        getTaskStatus(props.gid),
        getTaskFiles(props.gid)
      ])
      task.value = t
      files.value = f
    } catch {
      task.value = null
      files.value = []
    } finally {
      loading.value = false
    }
  }
)

const taskName = computed(() => {
  if (!task.value) return ''
  const btName = task.value.bittorrent?.info?.name
  if (btName) return btName
  const firstFile = task.value.files?.[0]?.path
  if (firstFile) {
    const parts = firstFile.replace(/\\/g, '/').split('/')
    return parts[parts.length - 1]
  }
  return task.value.gid
})

const progress = computed(() => {
  if (!task.value) return 0
  const total = task.value.totalLength || 0
  if (!total) return 0
  const done = task.value.completedLength || 0
  return Math.round((done / total) * 100)
})

const formatBytes = (value: number | string) => {
  const n = typeof value === 'number' ? value : parseInt(value || '0')
  if (n >= 1024 * 1024 * 1024) return (n / 1024 / 1024 / 1024).toFixed(2) + ' GB'
  if (n >= 1024 * 1024) return (n / 1024 / 1024).toFixed(2) + ' MB'
  if (n >= 1024) return (n / 1024).toFixed(2) + ' KB'
  return n + ' B'
}

const formatSpeed = (value: number | string) => {
  const n = typeof value === 'number' ? value : parseInt(value || '0')
  if (!n) return '0 B/s'
  return formatBytes(n) + '/s'
}

const isBt = computed(() => !!task.value?.bittorrent)

const onClose = () => emit('update:visible', false)
</script>

<template>
  <a-drawer
    :visible='visible'
    :width='480'
    :title='taskName || "任务详情"'
    :footer='false'
    @cancel='onClose'
  >
    <div v-if='loading' class='task-detail-loading'>
      <a-spin />
    </div>
    <div v-else-if='task' class='task-detail'>
      <a-descriptions :column='2' size='small' bordered>
        <a-descriptions-item label='状态'>{{ task.status }}</a-descriptions-item>
        <a-descriptions-item label='进度'>{{ progress }}%</a-descriptions-item>
        <a-descriptions-item label='已下载'>{{ formatBytes(task.completedLength) }}</a-descriptions-item>
        <a-descriptions-item label='总大小'>{{ formatBytes(task.totalLength) }}</a-descriptions-item>
        <a-descriptions-item label='下载速度'>{{ formatSpeed(task.downloadSpeed) }}</a-descriptions-item>
        <a-descriptions-item label='上传速度'>{{ formatSpeed(task.uploadSpeed) }}</a-descriptions-item>
        <template v-if='isBt'>
          <a-descriptions-item label='做种数'>{{ task.numSeeders || '0' }}</a-descriptions-item>
          <a-descriptions-item label='连接数'>{{ task.connections || '0' }}</a-descriptions-item>
          <a-descriptions-item v-if='task.bittorrent?.infoHash' label='InfoHash' :span='2'>
            <span class='hash-text'>{{ task.bittorrent.infoHash }}</span>
          </a-descriptions-item>
        </template>
        <a-descriptions-item label='保存路径' :span='2'>{{ task.dir }}</a-descriptions-item>
      </a-descriptions>

      <div v-if='files.length' class='task-files'>
        <div class='task-files-title'>文件列表</div>
        <div v-for='f in files' :key='f.index' class='task-file-item'>
          <span class='task-file-name'>{{ f.path.split('/').pop() || f.path }}</span>
          <span class='task-file-size'>{{ formatBytes(f.length) }}</span>
        </div>
      </div>
    </div>
    <div v-else class='task-detail-empty'>暂无数据</div>
  </a-drawer>
</template>

<style scoped>
.task-detail-loading {
  display: flex;
  justify-content: center;
  padding: 40px;
}
.task-detail-empty {
  text-align: center;
  color: var(--color-text-3);
  padding: 40px;
}
.hash-text {
  font-family: monospace;
  font-size: 11px;
  word-break: break-all;
}
.task-files {
  margin-top: 16px;
}
.task-files-title {
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--color-text-1);
}
.task-file-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 0;
  border-bottom: 1px solid var(--color-border-2);
  font-size: 13px;
}
.task-file-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-right: 8px;
}
.task-file-size {
  color: var(--color-text-3);
  white-space: nowrap;
}
</style>
