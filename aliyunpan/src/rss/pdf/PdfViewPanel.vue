<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { ChevronLeft, ChevronRight, ZoomIn, ZoomOut, RotateCw, Copy, Download, FileText } from 'lucide-vue-next'
import message from '../../utils/message'

export type ViewTab = 'pdf' | 'annot' | 'preview' | 'html' | 'md' | 'json'

const props = defineProps<{
  pdfDoc: any | null
  pdfName: string
  resultJson: any
  resultHtml: string
  resultMd: string
  resultText: string
  defaultTab: ViewTab
  loading: boolean
}>()

const emit = defineEmits<{
  (e: 'download', tab: ViewTab): void
}>()

const tabs: Array<{ key: ViewTab; label: string }> = [
  { key: 'pdf', label: 'PDF' },
  { key: 'annot', label: 'Annot' },
  { key: 'preview', label: 'Preview' },
  { key: 'html', label: 'HTML' },
  { key: 'md', label: 'MD' },
  { key: 'json', label: 'JSON' }
]

const activeTab = ref<ViewTab>(props.defaultTab)
const pageNum = ref(1)
const scale = ref(1)
const rotation = ref(0)
const canvasRef = ref<HTMLCanvasElement>()
const containerRef = ref<HTMLDivElement>()
const viewportSize = ref({ width: 0, height: 0 })
let renderTask: any = null

const pageCount = computed(() => Number(props.pdfDoc?.numPages || 0))
const isPdfTab = computed(() => activeTab.value === 'pdf' || activeTab.value === 'annot')

const pageBoxes = computed(() => {
  const json = props.resultJson
  if (!json || activeTab.value !== 'annot') return []
  const list: Array<{ type: string; bbox: number[]; content: string; level?: number }> = []
  const visit = (node: any) => {
    if (!node || typeof node !== 'object') return
    if (Array.isArray(node)) { node.forEach(visit); return }
    const pn = Number(node['page number'] ?? node.page_number ?? node.page)
    const bbox = node['bounding box'] || node.bounding_box || node.bbox
    if (pn === pageNum.value && Array.isArray(bbox) && bbox.length === 4) {
      list.push({
        type: String(node.type || 'item'),
        bbox: bbox.map((v: any) => Number(v) || 0),
        content: String(node.content || ''),
        level: Number(node['heading level'] || node.heading_level || 0)
      })
    }
    if (node.kids) visit(node.kids)
    if (node.children) visit(node.children)
  }
  visit(json)
  return list
})

const COLORS: { [k: string]: string } = {
  heading: 'rgba(120, 100, 220, 0.32)',
  paragraph: 'rgba(120, 200, 220, 0.32)',
  table: 'rgba(220, 100, 120, 0.32)',
  list: 'rgba(220, 180, 100, 0.32)',
  image: 'rgba(220, 100, 220, 0.32)',
  caption: 'rgba(100, 220, 160, 0.32)',
  footnote: 'rgba(160, 160, 160, 0.32)',
  item: 'rgba(150, 150, 220, 0.32)'
}

const colorOf = (type: string) => COLORS[type] || COLORS.item

const renderPage = async () => {
  if (!props.pdfDoc || !canvasRef.value) return
  try {
    if (renderTask) { renderTask.cancel?.(); renderTask = null }
    const page = await props.pdfDoc.getPage(pageNum.value)
    const viewport = page.getViewport({ scale: scale.value, rotation: rotation.value })
    const canvas = canvasRef.value
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    canvas.width = Math.floor(viewport.width)
    canvas.height = Math.floor(viewport.height)
    viewportSize.value = { width: viewport.width, height: viewport.height }
    renderTask = page.render({ canvasContext: ctx, viewport })
    await renderTask.promise
  } catch (err: any) {
    if (err?.name !== 'RenderingCancelledException') {
      message.error(err?.message || 'PDF 渲染失败')
    }
  }
}

watch(
  [() => props.pdfDoc, pageNum, scale, rotation, activeTab],
  () => {
    if (isPdfTab.value && props.pdfDoc) nextTick(renderPage)
  },
  { immediate: true }
)

watch(() => props.pdfDoc, () => { pageNum.value = 1 })

const prevPage = () => { if (pageNum.value > 1) pageNum.value -= 1 }
const nextPage = () => { if (pageNum.value < pageCount.value) pageNum.value += 1 }
const zoomOut = () => { scale.value = Math.max(0.4, Number((scale.value - 0.15).toFixed(2))) }
const zoomIn = () => { scale.value = Math.min(3, Number((scale.value + 0.15).toFixed(2))) }
const rotate = () => { rotation.value = (rotation.value + 90) % 360 }

const currentText = computed(() => {
  switch (activeTab.value) {
    case 'json': return props.resultJson ? JSON.stringify(props.resultJson, null, 2) : ''
    case 'html': return props.resultHtml || ''
    case 'md': return props.resultMd || ''
    case 'preview': return props.resultText || ''
    case 'annot': return props.resultText || ''
    default: return ''
  }
})

const copyContent = async () => {
  const text = currentText.value
  if (!text) { message.info('暂无可复制内容'); return }
  try {
    await navigator.clipboard.writeText(text)
    message.success('已复制')
  } catch {
    message.error('复制失败')
  }
}

const handleDownload = () => emit('download', activeTab.value)

onBeforeUnmount(() => { if (renderTask) renderTask.cancel?.() })

defineExpose({ activeTab, pageNum, scale, rotation })
</script>

<template>
  <div class="pdf-panel">
    <div class="pdf-panel-header">
      <div class="pdf-panel-tabs">
        <button
          v-for="t in tabs"
          :key="t.key"
          :class="['pdf-tab', { active: activeTab === t.key }]"
          @click="activeTab = t.key"
        >{{ t.label }}</button>
      </div>
      <div class="pdf-panel-actions">
        <button class="pdf-icon-btn" title="下载" @click="handleDownload">
          <Download :size="16" />
        </button>
      </div>
    </div>

    <div v-if="isPdfTab && pdfDoc" class="pdf-panel-toolbar">
      <div class="pdf-toolbar-group">
        <span class="pdf-page-info">{{ pageNum }} / {{ pageCount || '-' }}</span>
      </div>
      <div class="pdf-toolbar-group">
        <button class="pdf-icon-btn" :disabled="pageNum <= 1" title="上一页" @click="prevPage"><ChevronLeft :size="16" /></button>
        <button class="pdf-icon-btn" :disabled="pageNum >= pageCount" title="下一页" @click="nextPage"><ChevronRight :size="16" /></button>
        <span class="pdf-toolbar-divider"></span>
        <button class="pdf-icon-btn" title="缩小" @click="zoomOut"><ZoomOut :size="16" /></button>
        <button class="pdf-icon-btn" title="放大" @click="zoomIn"><ZoomIn :size="16" /></button>
        <button class="pdf-icon-btn" title="旋转" @click="rotate"><RotateCw :size="16" /></button>
        <span class="pdf-toolbar-divider"></span>
        <button class="pdf-icon-btn" title="复制" @click="copyContent"><Copy :size="16" /></button>
      </div>
    </div>

    <div v-else-if="!isPdfTab" class="pdf-panel-toolbar pdf-panel-toolbar-text">
      <span class="pdf-tab-label">{{ tabs.find(t => t.key === activeTab)?.label }}</span>
      <button class="pdf-icon-btn" title="复制" @click="copyContent"><Copy :size="16" /></button>
    </div>

    <div ref="containerRef" class="pdf-panel-body">
      <div v-if="loading" class="pdf-loading">
        <a-spin :size="28" />
        <span>正在转换 PDF…</span>
      </div>

      <template v-else>
        <!-- PDF / Annot 视图 -->
        <div v-if="isPdfTab" class="pdf-canvas-wrap">
          <div v-if="!pdfDoc" class="pdf-empty"><FileText :size="48" /><span>暂无 PDF</span></div>
          <div v-else class="pdf-canvas-stage">
            <canvas ref="canvasRef" class="pdf-canvas" />
            <svg
              v-if="activeTab === 'annot' && viewportSize.width > 0"
              class="pdf-annot-overlay"
              :viewBox="`0 0 ${viewportSize.width} ${viewportSize.height}`"
              preserveAspectRatio="none"
            >
              <g v-for="(box, idx) in pageBoxes" :key="idx">
                <rect
                  :x="box.bbox[0] * scale"
                  :y="viewportSize.height - box.bbox[3] * scale"
                  :width="(box.bbox[2] - box.bbox[0]) * scale"
                  :height="(box.bbox[3] - box.bbox[1]) * scale"
                  :fill="colorOf(box.type)"
                  stroke="rgba(80, 80, 80, 0.5)"
                  stroke-width="0.5"
                >
                  <title>{{ box.type }}{{ box.content ? ' · ' + box.content.slice(0, 80) : '' }}</title>
                </rect>
              </g>
            </svg>
          </div>
        </div>

        <!-- HTML / Preview 视图 -->
        <div v-else-if="activeTab === 'html'" class="pdf-html-render" v-html="resultHtml || '<div class=\'pdf-empty-text\'>暂无 HTML 内容</div>'"></div>

        <!-- MD 视图 -->
        <pre v-else-if="activeTab === 'md'" class="pdf-text-block">{{ resultMd || '暂无 Markdown 内容' }}</pre>

        <!-- Preview 视图 -->
        <pre v-else-if="activeTab === 'preview'" class="pdf-text-block">{{ resultText || '暂无文本内容' }}</pre>

        <!-- JSON 视图 -->
        <pre v-else-if="activeTab === 'json'" class="pdf-json-block"><code>{{ currentText || '暂无 JSON 数据' }}</code></pre>
      </template>
    </div>
  </div>
</template>

<style scoped>
.pdf-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  background: var(--color-bg-1);
}

.pdf-panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border-2);
  background: var(--color-bg-2);
  flex-shrink: 0;
}
.pdf-panel-tabs {
  display: flex;
  gap: 2px;
}
.pdf-tab {
  padding: 5px 12px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--color-text-3);
  font-size: 13px;
  cursor: pointer;
  font-weight: 500;
  transition: background 0.15s, color 0.15s;
}
.pdf-tab:hover {
  background: var(--color-fill-2);
  color: var(--color-text-1);
}
.pdf-tab.active {
  background: var(--color-fill-3);
  color: var(--color-text-1);
  font-weight: 600;
}
.pdf-panel-actions {
  display: flex;
  gap: 4px;
}

.pdf-panel-toolbar {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 6px 12px;
  border-bottom: 1px solid var(--color-border-2);
  background: var(--color-bg-1);
  flex-shrink: 0;
}
.pdf-panel-toolbar-text {
  justify-content: space-between;
}
.pdf-toolbar-group {
  display: flex;
  align-items: center;
  gap: 4px;
}
.pdf-toolbar-divider {
  width: 1px;
  height: 16px;
  background: var(--color-border-2);
  margin: 0 4px;
}
.pdf-page-info {
  font-size: 12px;
  color: var(--color-text-2);
  min-width: 48px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}
.pdf-tab-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-3);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.pdf-icon-btn {
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--color-text-2);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s, color 0.15s;
}
.pdf-icon-btn:hover:not(:disabled) {
  background: var(--color-fill-2);
  color: var(--color-text-1);
}
.pdf-icon-btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.pdf-panel-body {
  flex: 1;
  overflow: auto;
  background: var(--color-fill-1);
  position: relative;
}

.pdf-loading {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--color-text-3);
  font-size: 13px;
}

.pdf-canvas-wrap {
  display: flex;
  justify-content: center;
  padding: 16px;
  min-height: 100%;
}
.pdf-canvas-stage {
  position: relative;
  display: inline-block;
  background: #fff;
  box-shadow: 0 1px 8px rgba(15, 23, 42, 0.12);
}
.pdf-canvas {
  display: block;
}
.pdf-annot-overlay {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}
.pdf-annot-overlay rect {
  pointer-events: all;
}
.pdf-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 80px 0;
  color: var(--color-text-4);
  width: 100%;
}

.pdf-html-render {
  padding: 24px 28px;
  font-size: 14px;
  line-height: 1.7;
  color: var(--color-text-1);
  background: #fff;
}
.pdf-html-render :deep(img) { max-width: 100%; }
.pdf-html-render :deep(table) { border-collapse: collapse; margin: 8px 0; }
.pdf-html-render :deep(td), .pdf-html-render :deep(th) {
  border: 1px solid var(--color-border-2);
  padding: 4px 8px;
}

.pdf-text-block {
  margin: 0;
  padding: 16px 20px;
  font-size: 13px;
  line-height: 1.65;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: 'SF Mono', 'Menlo', 'Monaco', monospace;
  color: var(--color-text-1);
  background: #fff;
}

.pdf-json-block {
  margin: 0;
  padding: 16px 20px;
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
  font-family: 'SF Mono', 'Menlo', 'Monaco', monospace;
  color: var(--color-text-2);
  background: #fff;
}
.pdf-json-block code {
  font-family: inherit;
}

.pdf-empty-text {
  padding: 60px 0;
  text-align: center;
  color: var(--color-text-4);
  font-size: 13px;
}
</style>
