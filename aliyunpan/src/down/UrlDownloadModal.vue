<script setup lang="ts">
import { computed, reactive, ref, onMounted } from 'vue'
import fs from 'fs'
import path from 'path'
import { Clock3, Folder, Trash2, UploadCloud, X } from 'lucide-vue-next'
import DownDAL from './DownDAL'
import { useSettingStore } from '../store'
import message from '../utils/message'
import { parseExternalDownloadPayload } from './integration/protocolPayload'
import { parseTorrentMeta, type ParsedTorrentFile } from './integration/torrentMeta'

const props = defineProps({
  visible: {
    type: Boolean,
    required: true
  },
  initialUrl: {
    type: String,
    default: ''
  }
})

const emit = defineEmits(['update:visible'])

type ExternalSourceType = 'url' | 'magnet' | 'torrent' | 'torrent-url'
type TaskTab = 'uri' | 'torrent'

interface LocalTorrentItem {
  source: string
  fileName: string
  displayName: string
  torrentBase64: string
  files: ParsedTorrentFile[]
  selectedFileIndexes: number[]
}

interface DownloadEntry {
  source: string
  sourceType: ExternalSourceType
  fileName: string
  torrentBase64?: string
  selectFile?: string
}

const settingStore = useSettingStore()
const okLoading = ref(false)
const activeTab = ref<TaskTab>('uri')
const showAdvanced = ref(false)
const dragOver = ref(false)
const localTorrent = ref<LocalTorrentItem | null>(null)
const form = reactive({
  sources: '',
  fileName: '',
  savePath: '',
  split: 64,
  userAgent: '',
  authorization: '',
  referer: '',
  cookie: '',
  allProxy: '',
  newTaskShowDownloading: true
})

const maxSplit = computed(() => Math.max(1, settingStore.downThreadMax || 64, 64))
const selectedFileSet = computed(() => new Set(localTorrent.value?.selectedFileIndexes || []))

const defaultSavePath = () => {
  const ariaRemote = settingStore.ariaState === 'remote'
  return ariaRemote ? settingStore.ariaSavePath : settingStore.downSavePath
}

const resetForm = () => {
  form.sources = props.initialUrl || ''
  form.fileName = ''
  form.savePath = defaultSavePath()
  form.split = Math.max(1, settingStore.downThreadMax || 64)
  form.userAgent = ''
  form.authorization = ''
  form.referer = ''
  form.cookie = ''
  form.allProxy = ''
  form.newTaskShowDownloading = true
  localTorrent.value = null
  showAdvanced.value = false
  activeTab.value = props.initialUrl ? 'uri' : 'uri'
}

const isTorrentUrl = (source: string) => {
  if (!/^https?:\/\//i.test(source)) return false
  try {
    return decodeURIComponent(new URL(source).pathname).toLowerCase().endsWith('.torrent')
  } catch {
    return /\.torrent(?:\?|#|$)/i.test(source)
  }
}

const parseSourceLines = () => {
  return form.sources
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((source) => {
      const sourceType: ExternalSourceType = /^magnet:\?/i.test(source)
        ? 'magnet'
        : isTorrentUrl(source)
          ? 'torrent-url'
          : 'url'
      return { source, sourceType }
    })
}

const normalizeSplit = () => {
  const n = Number(form.split) || 1
  form.split = Math.min(Math.max(1, n), maxSplit.value)
}

const handleOpen = () => {
  resetForm()
}

const handleHide = () => {
  emit('update:visible', false)
}

const handleClose = () => {
  okLoading.value = false
  dragOver.value = false
}

const handleSelectSavePath = () => {
  if (!window.WebShowOpenDialogSync) return
  window.WebShowOpenDialogSync({
    title: '选择下载保存目录',
    defaultPath: form.savePath || defaultSavePath(),
    properties: ['openDirectory', 'createDirectory', 'showHiddenFiles', 'noResolveAliases', 'dontAddToRecent']
  }, (result: string[] | undefined) => {
    if (result && result[0]) form.savePath = result[0]
  })
}

const buildTorrentItem = (filePath: string): LocalTorrentItem => {
  const buf = fs.readFileSync(filePath)
  const torrentBase64 = buf.toString('base64')
  const parsed = parseTorrentMeta(buf)
  const displayName = parsed.name || path.basename(filePath)
  return {
    source: filePath,
    fileName: path.basename(filePath, path.extname(filePath)) || displayName || 'BT 种子任务',
    displayName,
    torrentBase64,
    files: parsed.files,
    selectedFileIndexes: parsed.files.map((file) => file.index)
  }
}

const loadTorrentFile = (filePath: string) => {
  try {
    localTorrent.value = buildTorrentItem(filePath)
    activeTab.value = 'torrent'
  } catch (e: any) {
    message.error('读取种子失败: ' + (e.message || filePath))
  }
}

const handleImportTorrent = () => {
  if (!window.WebShowOpenDialogSync) return
  window.WebShowOpenDialogSync({
    title: '选择 BT 种子文件',
    defaultPath: form.savePath || defaultSavePath(),
    filters: [{ name: 'Torrent', extensions: ['torrent'] }],
    properties: ['openFile', 'showHiddenFiles', 'noResolveAliases', 'dontAddToRecent']
  }, (result: string[] | undefined) => {
    if (result?.[0]) loadTorrentFile(result[0])
  })
}

const handleDrop = (event: DragEvent) => {
  dragOver.value = false
  const filePath = (event.dataTransfer?.files?.[0] as (File & { path?: string }) | undefined)?.path
  if (!filePath) return
  if (!filePath.toLowerCase().endsWith('.torrent')) {
    message.error('请先选择种子文件')
    return
  }
  loadTorrentFile(filePath)
}

const handleRemoveTorrent = () => {
  localTorrent.value = null
}

const toggleTorrentFile = (index: number) => {
  if (!localTorrent.value) return
  const selected = new Set(localTorrent.value.selectedFileIndexes)
  if (selected.has(index)) selected.delete(index)
  else selected.add(index)
  localTorrent.value.selectedFileIndexes = [...selected].sort((a, b) => a - b)
}

const toggleAllTorrentFiles = (checked: boolean) => {
  if (!localTorrent.value) return
  localTorrent.value.selectedFileIndexes = checked ? localTorrent.value.files.map((file) => file.index) : []
}

const formatBytes = (value: number | string) => {
  const n = typeof value === 'number' ? value : parseInt(value || '0')
  if (n >= 1024 * 1024 * 1024) return (n / 1024 / 1024 / 1024).toFixed(2) + ' GB'
  if (n >= 1024 * 1024) return (n / 1024 / 1024).toFixed(2) + ' MB'
  if (n >= 1024) return (n / 1024).toFixed(2) + ' KB'
  return n + ' B'
}

const validateSources = () => {
  if (activeTab.value === 'torrent') {
    if (!localTorrent.value) return '请先选择种子文件'
    if (!localTorrent.value.selectedFileIndexes.length) return '请至少选择一个种子文件'
  } else {
    const lines = parseSourceLines()
    for (const item of lines) {
      if (item.sourceType === 'magnet') continue
      if (!/^https?:\/\//i.test(item.source)) return '仅支持 http/https、magnet 链接或本地 .torrent 文件'
    }
    if (!lines.length) return '请输入下载链接'
  }
  if (!form.savePath.trim()) return '请选择保存目录'
  return ''
}

const handleCreate = async () => {
  const error = validateSources()
  if (error) {
    message.error(error)
    return
  }
  normalizeSplit()
  okLoading.value = true
  const entries: DownloadEntry[] = []
  if (activeTab.value === 'torrent' && localTorrent.value) {
    const selected = localTorrent.value.selectedFileIndexes
    entries.push({
      source: localTorrent.value.source,
      sourceType: 'torrent',
      fileName: form.fileName.trim() || localTorrent.value.displayName || localTorrent.value.fileName,
      torrentBase64: localTorrent.value.torrentBase64,
      selectFile: selected.length === localTorrent.value.files.length ? undefined : selected.join(',')
    })
  } else {
    entries.push(...parseSourceLines().map((item) => ({
      ...item,
      fileName: form.fileName.trim()
    })))
  }

  let successCount = 0
  for (const entry of entries) {
    const result = DownDAL.aAddExternalDownload({
      source: entry.source,
      sourceType: entry.sourceType,
      savePath: form.savePath.trim(),
      fileName: entries.length === 1 ? entry.fileName : '',
      torrentBase64: entry.torrentBase64,
      selectFile: entry.selectFile,
      split: form.split,
      userAgent: form.userAgent.trim(),
      authorization: form.authorization.trim(),
      referer: form.referer.trim(),
      cookie: form.cookie.trim(),
      allProxy: form.allProxy.trim()
    })
    if (result.success) successCount++
    else message.error(result.message || '创建下载任务失败')
  }
  okLoading.value = false
  if (successCount > 0) {
    message.success(`已创建 ${successCount} 个下载任务`)
    handleHide()
  }
}

onMounted(() => {
  if (typeof window !== 'undefined' && window.onExternalDownloadOpen) {
    window.onExternalDownloadOpen((payload: string) => {
      const parsed = parseExternalDownloadPayload(payload)
      if (!parsed) return
      resetForm()
      if (parsed.sourceType === 'torrent' && parsed.filePath) {
        loadTorrentFile(parsed.filePath)
        form.sources = ''
      } else {
        form.sources = parsed.source
        activeTab.value = 'uri'
      }
      form.savePath = defaultSavePath()
      emit('update:visible', true)
    })
  }
})
</script>

<template>
  <a-modal
    :visible="props.visible"
    modal-class="boxplayer-add-task-modal"
    :footer="false"
    :unmount-on-close="true"
    :mask-closable="false"
    :closable="false"
    :width="'67vw'"
    @cancel="handleHide"
    @before-open="handleOpen"
    @close="handleClose"
  >
    <div class="boxplayer-add-task">
      <button class="boxplayer-close" type="button" aria-label="Close" @click="handleHide">
        <X :size="20" />
      </button>
      <div class="boxplayer-tabs">
        <button
          class="boxplayer-tab"
          :class="{ active: activeTab === 'uri' }"
          type="button"
          @click="activeTab = 'uri'"
        >
          链接任务
        </button>
        <button
          class="boxplayer-tab"
          :class="{ active: activeTab === 'torrent' }"
          type="button"
          @click="activeTab = 'torrent'"
        >
          种子任务
        </button>
      </div>

      <div class="boxplayer-body">
        <div v-if="activeTab === 'uri'" class="uri-pane">
          <a-textarea
            v-model="form.sources"
            class="boxplayer-uri-input"
            :auto-size="{ minRows: 3, maxRows: 5 }"
            placeholder="添加多个下载链接时，请确保每行只有一个链接（支持磁力链）"
            @keydown.stop
          />
        </div>
        <div v-else class="torrent-pane">
          <div
            v-if="!localTorrent"
            class="torrent-drop"
            :class="{ dragging: dragOver }"
            @click="handleImportTorrent"
            @dragover.prevent="dragOver = true"
            @dragleave.prevent="dragOver = false"
            @drop.prevent="handleDrop"
          >
            <UploadCloud :size="20" />
            <span>将种子拖到此处，或点击选择</span>
          </div>
          <div v-else class="torrent-selected">
            <div class="torrent-info-row">
              <span class="torrent-name" :title="localTorrent.displayName">{{ localTorrent.displayName }}</span>
              <button class="icon-btn" type="button" aria-label="移除种子" @click="handleRemoveTorrent">
                <Trash2 :size="16" />
              </button>
            </div>
            <div class="torrent-file-head">
              <a-checkbox
                :model-value="localTorrent.selectedFileIndexes.length === localTorrent.files.length"
                :indeterminate="localTorrent.selectedFileIndexes.length > 0 && localTorrent.selectedFileIndexes.length < localTorrent.files.length"
                @change="(checked:boolean) => toggleAllTorrentFiles(checked)"
              />
              <span class="torrent-file-title">文件名</span>
              <span class="torrent-file-size">大小</span>
            </div>
            <div class="torrent-file-list">
              <div
                v-for="file in localTorrent.files"
                :key="file.index"
                class="torrent-file-item"
                @click="toggleTorrentFile(file.index)"
              >
                <a-checkbox :model-value="selectedFileSet.has(file.index)" @click.stop="toggleTorrentFile(file.index)" />
                <span class="torrent-file-title" :title="file.path">{{ file.path }}</span>
                <span class="torrent-file-size">{{ formatBytes(file.length) }}</span>
              </div>
            </div>
          </div>
        </div>

        <div class="boxplayer-form-grid">
          <label>重命名:</label>
          <a-input v-model.trim="form.fileName" placeholder="选填" />
          <label>分片数:</label>
          <a-input-number
            v-model="form.split"
            :min="1"
            :max="maxSplit"
            mode="button"
            @blur="normalizeSplit"
          />
        </div>

        <div class="boxplayer-dir-row">
          <label>存储路径:</label>
          <div class="boxplayer-dir-input">
            <button class="dir-prefix" type="button" aria-label="历史路径">
              <Clock3 :size="18" />
            </button>
            <a-input v-model="form.savePath" readonly />
            <button class="dir-suffix" type="button" aria-label="选择目录" @click="handleSelectSavePath">
              <Folder :size="20" />
            </button>
          </div>
        </div>

        <div v-if="showAdvanced" class="boxplayer-advanced">
          <div class="advanced-row">
            <label>User-Agent:</label>
            <a-textarea v-model="form.userAgent" :auto-size="{ minRows: 2, maxRows: 3 }" />
          </div>
          <div class="advanced-row">
            <label>Authorization:</label>
            <a-textarea v-model="form.authorization" :auto-size="{ minRows: 2, maxRows: 3 }" />
          </div>
          <div class="advanced-row">
            <label>Referer:</label>
            <a-textarea v-model="form.referer" :auto-size="{ minRows: 2, maxRows: 3 }" />
          </div>
          <div class="advanced-row">
            <label>Cookie:</label>
            <a-textarea v-model="form.cookie" :auto-size="{ minRows: 2, maxRows: 3 }" />
          </div>
          <div class="advanced-row">
            <label>代理:</label>
            <a-input v-model="form.allProxy" placeholder="[http://][USER:PASSWORD@]HOST[:PORT]" />
          </div>
          <div class="advanced-row advanced-check">
            <span></span>
            <a-checkbox v-model="form.newTaskShowDownloading">提交后跳转到下载中</a-checkbox>
          </div>
        </div>
      </div>

      <div class="boxplayer-footer">
        <a-checkbox v-model="showAdvanced">高级选项</a-checkbox>
        <div class="boxplayer-footer-actions">
          <a-button size="small" @click="handleHide">取 消</a-button>
          <a-button size="small" type="primary" :loading="okLoading" @click="handleCreate">提 交</a-button>
        </div>
      </div>
    </div>
  </a-modal>
</template>

<style scoped>
:global(.boxplayer-add-task-modal) {
  max-width: 632px;
  min-width: 380px;
  padding: 0;
  overflow: hidden;
  border-radius: 5px;
}

:global(.boxplayer-add-task-modal .arco-modal-header) {
  display: none;
}

:global(.boxplayer-add-task-modal .arco-modal-body) {
  padding: 0;
}

.boxplayer-add-task {
  position: relative;
  color: var(--color-text-2);
  background: var(--color-bg-1);
}

.boxplayer-close {
  position: absolute;
  top: 24px;
  right: 24px;
  z-index: 2;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  color: var(--color-text-3);
  background: transparent;
  border: 0;
  cursor: pointer;
}

.boxplayer-tabs {
  display: flex;
  gap: 48px;
  margin: 0 24px;
  padding-top: 24px;
  border-bottom: 2px solid var(--color-border-2);
}

.boxplayer-tab {
  position: relative;
  height: 40px;
  padding: 0;
  color: var(--color-text-1);
  font-size: 17px;
  font-weight: 600;
  line-height: 40px;
  background: transparent;
  border: 0;
  cursor: pointer;
}

.boxplayer-tab.active {
  color: rgb(var(--primary-6));
}

.boxplayer-tab.active::after {
  position: absolute;
  right: 0;
  bottom: -2px;
  left: 0;
  height: 3px;
  background: rgb(var(--primary-6));
  content: '';
}

.boxplayer-body {
  padding: 20px 24px 24px;
}

.uri-pane {
  margin-bottom: 16px;
}

.boxplayer-uri-input :deep(textarea) {
  min-height: 100px;
  padding: 10px 12px;
  color: var(--color-text-2);
  font-size: 14px;
  font-weight: 400;
  line-height: 22px;
  border-color: var(--color-border-2);
  border-radius: 5px;
  resize: none;
}

.boxplayer-uri-input :deep(textarea:focus),
.boxplayer-uri-input :deep(textarea.arco-textarea-focus) {
  border-color: rgb(var(--primary-6));
}

.boxplayer-uri-input :deep(textarea::placeholder) {
  color: var(--color-text-4);
}

.torrent-drop {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100px;
  gap: 8px;
  color: var(--color-text-3);
  font-size: 14px;
  border: 2px dashed var(--color-border-2);
  border-radius: 5px;
  cursor: pointer;
}

.torrent-drop.dragging,
.torrent-drop:hover {
  color: rgb(var(--primary-6));
  border-color: rgb(var(--primary-6));
}

.torrent-selected {
  min-height: 100px;
}

.torrent-info-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 32px;
  align-items: center;
  margin-bottom: 8px;
  color: var(--color-text-2);
  font-size: 14px;
}

.torrent-name,
.torrent-file-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  color: var(--color-text-3);
  background: transparent;
  border: 0;
  cursor: pointer;
}

.torrent-file-head,
.torrent-file-item {
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr) 96px;
  align-items: center;
  gap: 8px;
  min-height: 32px;
  padding: 0 8px;
  font-size: 12px;
}

.torrent-file-head {
  color: var(--color-text-3);
  background: var(--color-fill-1);
  border: 1px solid var(--color-border-2);
}

.torrent-file-list {
  max-height: 168px;
  overflow-y: auto;
  border: 1px solid var(--color-border-2);
  border-top: 0;
}

.torrent-file-item {
  color: var(--color-text-2);
  cursor: pointer;
}

.torrent-file-item:hover {
  background: var(--color-fill-1);
}

.torrent-file-size {
  color: var(--color-text-3);
  text-align: right;
  white-space: nowrap;
}

.boxplayer-form-grid {
  display: grid;
  grid-template-columns: 80px minmax(0, 1fr) 72px 180px;
  align-items: center;
  gap: 16px 12px;
  margin-top: 20px;
}

.boxplayer-form-grid label,
.boxplayer-dir-row label,
.advanced-row label {
  color: var(--color-text-2);
  font-size: 14px;
  font-weight: 500;
}

.boxplayer-form-grid :deep(.arco-input-wrapper),
.boxplayer-form-grid :deep(.arco-input-number),
.boxplayer-dir-input :deep(.arco-input-wrapper) {
  height: auto;
  color: var(--color-text-2);
  font-size: 14px;
  background: var(--color-bg-2);
  border-color: var(--color-border-2);
  border-radius: 5px;
}

.boxplayer-form-grid :deep(.arco-input),
.boxplayer-dir-input :deep(.arco-input) {
  font-size: 14px;
  font-weight: 400;
}

.boxplayer-form-grid :deep(.arco-input::placeholder) {
  color: var(--color-text-4);
}

.boxplayer-form-grid :deep(.arco-input-number) {
  width: 180px;
}

.boxplayer-dir-row {
  display: grid;
  grid-template-columns: 160px minmax(0, 1fr);
  align-items: center;
  margin-top: 16px;
}

.boxplayer-dir-input {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr) 48px;
}

.dir-prefix,
.dir-suffix {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 36px;
  color: var(--color-text-3);
  background: var(--color-fill-1);
  border: 1px solid var(--color-border-2);
  cursor: pointer;
}

.dir-prefix {
  border-radius: 5px 0 0 5px;
}

.dir-suffix {
  border-radius: 0 5px 5px 0;
}

.boxplayer-dir-input :deep(.arco-input-wrapper) {
  border-right: 0;
  border-left: 0;
  border-radius: 0;
}

.boxplayer-advanced {
  display: grid;
  gap: 8px;
  margin-top: 16px;
}

.advanced-row {
  display: grid;
  grid-template-columns: 160px minmax(0, 1fr);
  align-items: center;
}

.advanced-check {
  align-items: center;
}

.boxplayer-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 56px;
  padding: 0 24px;
  background: var(--color-fill-1);
}

.boxplayer-footer :deep(.arco-checkbox-label) {
  color: var(--color-text-2);
  font-size: 14px;
  font-weight: 400;
}

.boxplayer-footer-actions {
  display: flex;
  gap: 12px;
}

.boxplayer-footer-actions :deep(.arco-btn-primary) {
  background: rgb(var(--primary-6));
}

@media (max-width: 720px) {
  :global(.boxplayer-add-task-modal) {
    width: calc(100vw - 24px) !important;
    min-width: 0;
  }

  .boxplayer-tabs {
    gap: 24px;
    margin: 0 16px;
  }

  .boxplayer-body,
  .boxplayer-footer {
    padding-right: 16px;
    padding-left: 16px;
  }

  .boxplayer-form-grid,
  .boxplayer-dir-row,
  .advanced-row {
    grid-template-columns: 1fr;
    gap: 8px;
  }

  .boxplayer-dir-input {
    grid-template-columns: 40px minmax(0, 1fr) 40px;
  }
}
</style>
