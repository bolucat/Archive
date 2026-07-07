<script setup lang='ts'>
import { ref, watch } from 'vue'
import { getTaskFiles, changeTaskSelectedFiles } from './integration/aria2TaskApi'
import type { DownloadTaskFile } from './integration/taskTypes'

const props = defineProps<{
  visible: boolean
  gid: string
}>()
const emit = defineEmits<{
  (e: 'update:visible', val: boolean): void
  (e: 'confirmed'): void
}>()

const files = ref<DownloadTaskFile[]>([])
const selected = ref<Set<number>>(new Set())
const loading = ref(false)
const saving = ref(false)

watch(
  () => props.visible,
  async (v) => {
    if (!v || !props.gid) return
    loading.value = true
    try {
      files.value = await getTaskFiles(props.gid)
      selected.value = new Set(
        files.value.filter((f) => f.selected).map((f) => f.index)
      )
    } catch {
      files.value = []
    } finally {
      loading.value = false
    }
  }
)

const formatBytes = (value: number | string) => {
  const n = typeof value === 'number' ? value : parseInt(value || '0')
  if (n >= 1024 * 1024 * 1024) return (n / 1024 / 1024 / 1024).toFixed(2) + ' GB'
  if (n >= 1024 * 1024) return (n / 1024 / 1024).toFixed(2) + ' MB'
  if (n >= 1024) return (n / 1024).toFixed(2) + ' KB'
  return n + ' B'
}

const toggleFile = (index: number) => {
  if (selected.value.has(index)) selected.value.delete(index)
  else selected.value.add(index)
}

const onConfirm = async () => {
  if (!props.gid) return
  saving.value = true
  try {
    await changeTaskSelectedFiles(props.gid, [...selected.value])
    emit('confirmed')
    emit('update:visible', false)
  } catch {
  } finally {
    saving.value = false
  }
}

const onClose = () => emit('update:visible', false)
</script>

<template>
  <a-modal
    :visible='visible'
    title='选择下载文件'
    :ok-loading='saving'
    ok-text='确认'
    cancel-text='取消'
    @ok='onConfirm'
    @cancel='onClose'
  >
    <div v-if='loading' class='torrent-loading'>
      <a-spin />
    </div>
    <div v-else-if='files.length' class='torrent-file-list'>
      <div
        v-for='f in files'
        :key='f.index'
        class='torrent-file-item'
        @click='toggleFile(f.index)'
      >
        <a-checkbox :model-value='selected.has(f.index)' @click.stop='toggleFile(f.index)' />
        <span class='torrent-file-name'>{{ f.path.split('/').pop() || f.path }}</span>
        <span class='torrent-file-size'>{{ formatBytes(f.length) }}</span>
      </div>
    </div>
    <div v-else class='torrent-empty'>暂无文件信息</div>
  </a-modal>
</template>

<style scoped>
.torrent-loading,
.torrent-empty {
  text-align: center;
  padding: 24px;
  color: var(--color-text-3);
}
.torrent-file-list {
  max-height: 400px;
  overflow-y: auto;
}
.torrent-file-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 4px;
  border-radius: 4px;
  cursor: pointer;
}
.torrent-file-item:hover {
  background: var(--color-fill-2);
}
.torrent-file-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}
.torrent-file-size {
  color: var(--color-text-3);
  white-space: nowrap;
  font-size: 12px;
}
</style>
