<script setup lang="ts">
import { computed, markRaw, onMounted, ref, shallowRef, watch } from 'vue'
import * as pdfjsLib from 'pdfjs-dist'
import pdfWorkerUrl from 'pdfjs-dist/build/pdf.worker.js?url'
import { FileText, FolderOpen, Plus, RefreshCw, Trash2, X, PanelLeftClose, PanelLeftOpen } from 'lucide-vue-next'
import { useSettingStore } from '../../store'
import message from '../../utils/message'
import PdfViewPanel from './PdfViewPanel.vue'

;(pdfjsLib as any).GlobalWorkerOptions.workerSrc = pdfWorkerUrl

interface PdfFileItem {
  id: string
  path: string
  name: string
  addedAt: number
  lastUsedAt: number
}

interface ConvertResult {
  json: any
  html: string
  md: string
  text: string
  files?: Array<{ path: string; name: string; format: string }>
}

const STORAGE_KEY = 'box.pdfTools.files.v1'

const fileList = ref<PdfFileItem[]>([])
const currentId = ref<string>('')
const sidebarCollapsed = ref(false)
const converting = ref(false)
const pdfDoc = shallowRef<any>(null)
const loadingPdf = ref(false)
const resultCache = new Map<string, ConvertResult>()
const resultJson = ref<any>(null)
const resultHtml = ref('')
const resultMd = ref('')
const resultText = ref('')

const current = computed(() => fileList.value.find((f) => f.id === currentId.value) || null)

const loadFiles = () => {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw)
      if (Array.isArray(parsed)) fileList.value = parsed.filter((f) => f && f.path && f.name)
    }
  } catch {}
}

const saveFiles = () => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(fileList.value))
  } catch {}
}

const genId = () => Math.random().toString(36).slice(2, 10) + Date.now().toString(36)

const addFile = (filePath: string) => {
  const name = filePath.replace(/\\/g, '/').split('/').pop() || filePath
  const exists = fileList.value.find((f) => f.path === filePath)
  if (exists) {
    exists.lastUsedAt = Date.now()
    currentId.value = exists.id
  } else {
    const item: PdfFileItem = { id: genId(), path: filePath, name, addedAt: Date.now(), lastUsedAt: Date.now() }
    fileList.value.unshift(item)
    currentId.value = item.id
  }
  saveFiles()
}

const removeFile = (id: string, ev?: Event) => {
  ev?.stopPropagation()
  const idx = fileList.value.findIndex((f) => f.id === id)
  if (idx < 0) return
  const removed = fileList.value.splice(idx, 1)[0]
  if (removed) resultCache.delete(removed.path)
  saveFiles()
  if (currentId.value === id) {
    const next = fileList.value[idx] || fileList.value[idx - 1] || null
    currentId.value = next?.id || ''
    if (!next) clearViewer()
  }
}

const clearViewer = () => {
  pdfDoc.value = null
  resultJson.value = null
  resultHtml.value = ''
  resultMd.value = ''
  resultText.value = ''
}

const handleSelectFile = () => {
  if (!window.WebShowOpenDialogSync) return
  window.WebShowOpenDialogSync(
    {
      title: '选择 PDF 文件',
      buttonLabel: '打开',
      properties: ['openFile', 'multiSelections'],
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
      defaultPath: useSettingStore().downSavePath
    },
    (paths: string[] | undefined) => {
      if (!paths || !paths.length) return
      paths.forEach((p) => addFile(p))
    }
  )
}

const loadPdfDocument = async (filePath: string) => {
  loadingPdf.value = true
  try {
    const normalized = filePath.replace(/\\/g, '/')
    const withSlash = normalized.startsWith('/') ? normalized : '/' + normalized
    const url = 'file://' + encodeURI(withSlash).replace(/#/g, '%23').replace(/\?/g, '%3F')
    const task = (pdfjsLib as any).getDocument({ url, withCredentials: false })
    const doc = await task.promise
    pdfDoc.value = markRaw(doc)
  } catch (err: any) {
    message.error(err?.message || 'PDF 加载失败')
    pdfDoc.value = null
  } finally {
    loadingPdf.value = false
  }
}

const runConvert = async (force = false) => {
  const f = current.value
  if (!f) return
  if (!force && resultCache.has(f.path)) {
    const cached = resultCache.get(f.path)!
    resultJson.value = cached.json
    resultHtml.value = cached.html
    resultMd.value = cached.md
    resultText.value = cached.text
    return
  }
  if (converting.value) return
  converting.value = true
  try {
    const payload = {
      inputPath: f.path,
      outputDir: useSettingStore().downSavePath || ''
    }
    const response = (await window.TvBoxInvoke('opendataloader:convertPdf', payload)) as { ok?: boolean; result?: any; error?: string; stderr?: string }
    console.log('[PdfTools] convertPdf response:', response)
    if (!response?.ok) {
      const rawErr = String(response?.error || '转换失败')
      const hint = rawErr.includes('请选择输出文件夹')
        ? '主进程仍在使用旧版代码,请完全退出应用后重新启动 (pnpm dev / 重新打开 app)'
        : rawErr
      message.error(hint + (response?.stderr ? '\n' + String(response.stderr).slice(0, 200) : ''))
      return
    }
    const r = response.result || {}
    const data: ConvertResult = {
      json: r.json ?? null,
      html: r.html || '',
      md: r.md || r.markdown || '',
      text: r.text || '',
      files: r.files || []
    }
    if (!data.json && !data.html && !data.md && !data.text) {
      console.warn('[PdfTools] empty conversion result. full response:', response)
      message.warning('转换完成但产物为空,请重启应用让主进程加载新代码,或打开 DevTools 查看 [PdfTools] 日志')
    }
    resultCache.set(f.path, data)
    resultJson.value = data.json
    resultHtml.value = data.html
    resultMd.value = data.md
    resultText.value = data.text
  } catch (err: any) {
    message.error(err?.message || '转换失败')
  } finally {
    converting.value = false
  }
}

watch(currentId, async (id) => {
  clearViewer()
  if (!id) return
  const f = current.value
  if (!f) return
  f.lastUsedAt = Date.now()
  saveFiles()
  await loadPdfDocument(f.path)
  await runConvert(false)
})

const handleDownload = (tab: string) => {
  const cached = current.value ? resultCache.get(current.value.path) : null
  if (!cached || !cached.files?.length) {
    message.info('暂无可下载的产物文件')
    return
  }
  const map: { [k: string]: string[] } = {
    json: ['.json'],
    html: ['.html', '.htm'],
    md: ['.md', '.markdown'],
    preview: ['.txt'],
    annot: ['.txt'],
    pdf: ['.pdf']
  }
  const exts = map[tab] || []
  const target = cached.files.find((f) => exts.some((e) => f.path.toLowerCase().endsWith(e)))
  if (!target) {
    message.info('该格式无产物文件')
    return
  }
  if (window.WebShowItemInFolder) window.WebShowItemInFolder(target.path)
  else message.success('文件位于: ' + target.path)
}

onMounted(() => {
  loadFiles()
})
</script>

<template>
  <div class="pdf-tools-page">
    <!-- 左侧文件列表 -->
    <aside class="pdf-tools-sidebar" :class="{ collapsed: sidebarCollapsed }">
      <div class="pdf-tools-sidebar-header">
        <span v-if="!sidebarCollapsed" class="pdf-tools-sidebar-title">PDF 文件</span>
        <button class="pdf-tools-side-toggle" :title="sidebarCollapsed ? '展开' : '收起'" @click="sidebarCollapsed = !sidebarCollapsed">
          <PanelLeftOpen v-if="sidebarCollapsed" :size="16" />
          <PanelLeftClose v-else :size="16" />
        </button>
      </div>

      <template v-if="!sidebarCollapsed">
        <button class="pdf-tools-add-btn" @click="handleSelectFile">
          <Plus :size="14" />
          <span>添加 PDF…</span>
        </button>

        <div class="pdf-tools-file-list">
          <div
            v-for="item in fileList"
            :key="item.id"
            :class="['pdf-tools-file-item', { active: item.id === currentId }]"
            @click="currentId = item.id"
          >
            <div class="pdf-tools-file-thumb">
              <FileText :size="22" />
            </div>
            <div class="pdf-tools-file-meta">
              <div class="pdf-tools-file-name" :title="item.path">{{ item.name }}</div>
            </div>
            <button class="pdf-tools-file-remove" title="移除" @click="removeFile(item.id, $event)">
              <X :size="14" />
            </button>
          </div>

          <div v-if="!fileList.length" class="pdf-tools-empty-list">
            <FolderOpen :size="28" />
            <span>暂无文件</span>
          </div>
        </div>
      </template>
    </aside>

    <!-- 右侧主内容 -->
    <main class="pdf-tools-main">
      <!-- 空状态 -->
      <div v-if="!current" class="pdf-tools-empty">
        <div class="pdf-tools-empty-icon"><FolderOpen :size="56" /></div>
        <h2 class="pdf-tools-empty-title">打开 PDF 文件</h2>
        <p class="pdf-tools-empty-desc">选择一个本地 PDF 文件，查看 PDF 渲染、结构标注、HTML、Markdown 与 JSON 数据</p>
        <button class="pdf-tools-empty-btn" @click="handleSelectFile">选择文件…</button>
      </div>

      <!-- 主操作面板 -->
      <template v-else>
        <div class="pdf-tools-topbar">
          <div class="pdf-tools-topbar-left">
            <FileText :size="16" />
            <span class="pdf-tools-topbar-name" :title="current.path">{{ current.name }}</span>
          </div>
          <div class="pdf-tools-topbar-right">
            <button class="pdf-tools-topbar-btn" :disabled="converting" title="重新转换" @click="runConvert(true)">
              <RefreshCw :size="14" :class="{ 'pdf-spin': converting }" />
              <span>{{ converting ? '转换中…' : '重新转换' }}</span>
            </button>
            <button class="pdf-tools-topbar-btn" title="移除" @click="removeFile(current.id)">
              <Trash2 :size="14" />
            </button>
          </div>
        </div>

        <div class="pdf-tools-workspace">
          <PdfViewPanel
            class="pdf-tools-pane pdf-tools-pane-left"
            :pdf-doc="pdfDoc"
            :pdf-name="current.name"
            :result-json="resultJson"
            :result-html="resultHtml"
            :result-md="resultMd"
            :result-text="resultText"
            :default-tab="'pdf'"
            :loading="converting || loadingPdf"
            @download="handleDownload"
          />
          <PdfViewPanel
            class="pdf-tools-pane pdf-tools-pane-right"
            :pdf-doc="pdfDoc"
            :pdf-name="current.name"
            :result-json="resultJson"
            :result-html="resultHtml"
            :result-md="resultMd"
            :result-text="resultText"
            :default-tab="'json'"
            :loading="converting || loadingPdf"
            @download="handleDownload"
          />
        </div>
      </template>
    </main>
  </div>
</template>

<style scoped>
.pdf-tools-page {
  display: flex;
  height: 100%;
  min-height: 0;
  background: var(--color-bg-1);
  color: var(--color-text-1);
}

/* ===== Sidebar ===== */
.pdf-tools-sidebar {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--color-border-2);
  background: var(--color-bg-2);
  transition: width 0.2s ease;
  overflow: hidden;
}
.pdf-tools-sidebar.collapsed {
  width: 44px;
}
.pdf-tools-sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 12px 8px;
  flex-shrink: 0;
}
.pdf-tools-sidebar-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--color-text-1);
}
.pdf-tools-side-toggle {
  width: 26px;
  height: 26px;
  border: 1px solid var(--color-border-2);
  border-radius: 5px;
  background: var(--color-bg-1);
  color: var(--color-text-2);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.pdf-tools-side-toggle:hover {
  color: rgb(var(--primary-6));
  border-color: rgb(var(--primary-6));
}

.pdf-tools-add-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  margin: 0 10px 8px;
  padding: 6px 0;
  border: 1px dashed var(--color-border-2);
  border-radius: 6px;
  background: transparent;
  color: var(--color-text-2);
  font-size: 13px;
  cursor: pointer;
  transition: border-color 0.15s, color 0.15s;
}
.pdf-tools-add-btn:hover {
  border-color: rgb(var(--primary-6));
  color: rgb(var(--primary-6));
}

.pdf-tools-file-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 8px 12px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.pdf-tools-file-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 7px 8px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--color-text-2);
  cursor: pointer;
  text-align: left;
  position: relative;
  transition: background 0.15s, border-color 0.15s;
}
.pdf-tools-file-item:hover {
  background: var(--color-fill-2);
}
.pdf-tools-file-item:hover .pdf-tools-file-remove {
  opacity: 1;
}
.pdf-tools-file-item.active {
  background: rgba(var(--primary-6), 0.12);
  color: rgb(var(--primary-6));
  border-color: rgba(var(--primary-6), 0.25);
}

.pdf-tools-file-thumb {
  width: 30px;
  height: 38px;
  border-radius: 4px;
  background: var(--color-bg-1);
  border: 1px solid var(--color-border-2);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: rgb(var(--primary-6));
  flex-shrink: 0;
}
.pdf-tools-file-item.active .pdf-tools-file-thumb {
  background: #fff;
}
.pdf-tools-file-meta {
  flex: 1;
  min-width: 0;
}
.pdf-tools-file-name {
  font-size: 12.5px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  line-height: 1.4;
}
.pdf-tools-file-item.active .pdf-tools-file-name {
  font-weight: 600;
}

.pdf-tools-file-remove {
  width: 22px;
  height: 22px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--color-text-3);
  cursor: pointer;
  opacity: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: opacity 0.15s, background 0.15s;
}
.pdf-tools-file-remove:hover {
  background: var(--color-fill-3);
  color: rgb(var(--danger-6));
}

.pdf-tools-empty-list {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 36px 0;
  color: var(--color-text-4);
  font-size: 12px;
}

/* ===== Main ===== */
.pdf-tools-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.pdf-tools-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 40px 20px;
}
.pdf-tools-empty-icon {
  color: var(--color-text-4);
  margin-bottom: 4px;
}
.pdf-tools-empty-title {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  color: var(--color-text-1);
}
.pdf-tools-empty-desc {
  margin: 0;
  max-width: 380px;
  text-align: center;
  font-size: 13px;
  color: var(--color-text-3);
  line-height: 1.5;
}
.pdf-tools-empty-btn {
  margin-top: 8px;
  padding: 9px 26px;
  border: none;
  border-radius: 8px;
  background: rgb(var(--primary-6));
  color: #fff;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}
.pdf-tools-empty-btn:hover {
  opacity: 0.9;
}

.pdf-tools-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--color-border-2);
  background: var(--color-bg-2);
  flex-shrink: 0;
}
.pdf-tools-topbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  color: var(--color-text-1);
}
.pdf-tools-topbar-name {
  font-size: 13px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pdf-tools-topbar-right {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
.pdf-tools-topbar-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 12px;
  border: 1px solid var(--color-border-2);
  border-radius: 6px;
  background: var(--color-bg-1);
  color: var(--color-text-2);
  font-size: 12.5px;
  cursor: pointer;
  transition: color 0.15s, border-color 0.15s;
}
.pdf-tools-topbar-btn:hover:not(:disabled) {
  color: rgb(var(--primary-6));
  border-color: rgb(var(--primary-6));
}
.pdf-tools-topbar-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.pdf-tools-workspace {
  flex: 1;
  display: flex;
  min-height: 0;
  overflow: hidden;
}
.pdf-tools-pane {
  min-width: 0;
}
.pdf-tools-pane-left {
  border-right: 1px solid var(--color-border-2);
}

.pdf-spin {
  animation: pdf-rotate 1s linear infinite;
}
@keyframes pdf-rotate {
  from { transform: rotate(0); }
  to { transform: rotate(360deg); }
}
</style>
