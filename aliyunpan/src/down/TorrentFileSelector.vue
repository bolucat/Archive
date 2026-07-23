<script setup lang='ts'>
import { computed, ref, watch } from 'vue'
import { getTaskFiles, changeTaskSelectedFiles } from './integration/aria2TaskApi'
import type { DownloadTaskFile } from './integration/taskTypes'
import { t } from '../i18n'

const props = withDefaults(defineProps<{
  visible: boolean
  gid?: string
  initialFiles?: DownloadTaskFile[]
  externalLoading?: boolean
  applyToTask?: boolean
}>(), {
  gid: '',
  initialFiles: undefined,
  externalLoading: false,
  applyToTask: true
})
const emit = defineEmits<{
  (e: 'update:visible', val: boolean): void
  (e: 'confirmed'): void
  (e: 'confirm-selection', indexes: number[]): void
  (e: 'direct-confirm'): void
}>()

const files = ref<DownloadTaskFile[]>([])
const selected = ref<Set<number>>(new Set())
const internalLoading = ref(false)
const saving = ref(false)
const loading = computed(() => props.externalLoading || internalLoading.value)

watch(
  () => [props.visible, props.initialFiles] as const,
  async ([v]) => {
    if (!v) return
    if (!props.applyToTask) {
      files.value = props.initialFiles || []
      selected.value = new Set(files.value.map((file) => file.index))
      return
    }
    if (!props.gid) return
    internalLoading.value = true
    try {
      files.value = await getTaskFiles(props.gid)
      selected.value = new Set(
        files.value.filter((f) => f.selected).map((f) => f.index)
      )
    } catch {
      files.value = []
    } finally {
      internalLoading.value = false
    }
  },
  { immediate: true }
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

const selectAll = () => {
  selected.value = new Set(files.value.map((file) => file.index))
}

const clearSelection = () => {
  selected.value = new Set()
}

const onConfirm = async () => {
  if (!selected.value.size) return
  const indexes = [...selected.value].sort((a, b) => a - b)
  if (!props.applyToTask) {
    emit('confirm-selection', indexes)
    emit('update:visible', false)
    return
  }
  if (!props.gid) return
  saving.value = true
  try {
    await changeTaskSelectedFiles(props.gid, indexes)
    emit('confirmed')
    emit('update:visible', false)
  } catch {
  } finally {
    saving.value = false
  }
}

const onClose = () => emit('update:visible', false)
const onDirectConfirm = () => emit('direct-confirm')
</script>

<template>
  <a-modal
    :visible='visible'
    :title="t('transfer.selectDownloadFiles')"
    :ok-loading='saving'
    :ok-button-props='{ disabled: loading || selected.size === 0 }'
    :ok-text="t('common.confirm')"
    :cancel-text="t('common.cancel')"
    @ok='onConfirm'
    @cancel='onClose'
  >
    <template v-if='!applyToTask' #footer>
      <a-space>
        <a-button @click='onClose'>{{ t('common.cancel') }}</a-button>
        <a-button :disabled='saving' @click='onDirectConfirm'>{{ t('transfer.addDirectly') }}</a-button>
        <a-button type='primary' :disabled='loading || selected.size === 0' :loading='saving' @click='onConfirm'>{{ t('transfer.addSelected') }}</a-button>
      </a-space>
    </template>
    <div v-if='loading' class='torrent-loading'>
      <a-spin />
      <div class='torrent-loading-text'>{{ t('transfer.loadingMagnetFiles') }}</div>
    </div>
    <div v-else-if='files.length' class='torrent-file-list'>
      <div class='torrent-file-toolbar'>
        <span>{{ t('transfer.selectedFilesCount', { selected: selected.size, total: files.length }) }}</span>
        <a-space>
          <a-button size='mini' type='text' @click='selectAll'>{{ t('transfer.selectAll') }}</a-button>
          <a-button size='mini' type='text' @click='clearSelection'>{{ t('common.clear') }}</a-button>
        </a-space>
      </div>
      <div
        v-for='f in files'
        :key='f.index'
        class='torrent-file-item'
        @click='toggleFile(f.index)'
      >
        <a-checkbox :model-value='selected.has(f.index)' @click.stop='toggleFile(f.index)' />
        <span class='torrent-file-name' :title='f.path'>{{ f.path.split('/').pop() || f.path }}</span>
        <span class='torrent-file-size'>{{ formatBytes(f.length) }}</span>
      </div>
    </div>
    <div v-else class='torrent-empty'>{{ t('transfer.noFileInfo') }}</div>
  </a-modal>
</template>

<style scoped>
.torrent-loading,
.torrent-empty {
  text-align: center;
  padding: 24px;
  color: var(--color-text-3);
}
.torrent-loading-text {
  margin-top: 10px;
  font-size: 12px;
}
.torrent-file-list {
  max-height: 400px;
  overflow-y: auto;
}
.torrent-file-toolbar {
  position: sticky;
  top: 0;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 32px;
  padding: 4px 6px;
  color: var(--color-text-3);
  background: var(--color-bg-2);
  border-bottom: 1px solid var(--color-border-2);
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
