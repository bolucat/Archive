<script setup lang='ts'>
import { computed, ref, watch } from 'vue'
import AliFile from '../aliapi/file'
import { createBookAISettings, getAIConfig } from '../utils/bookAI'
import { getAIProvider } from '../services/ai/providers'
import { askIndexedDocument, indexDocumentLocally } from '../services/documents'
import { canUseSemanticEmbeddings } from '../services/ai/embeddingPolicy'
import message from '../utils/message'

const props = defineProps<{
  visible: boolean
  file: any | null
  userId: string
}>()
const emit = defineEmits<{ (event: 'update:visible', value: boolean): void }>()

const indexing = ref(false)
const indexed = ref(false)
const asking = ref(false)
const status = ref('')
const question = ref('')
const answer = ref('')
const citations = ref<Array<{ location: string; section: string; text: string }>>([])
const abortController = ref<AbortController | null>(null)

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message
  if (typeof error === 'string' && error) return error
  try { return JSON.stringify(error) || '文档索引失败' } catch { return '文档索引失败' }
}

const fileName = computed(() => props.file?.name || props.file?.file_name || '文档')
const driveId = computed(() => props.file?.drive_id || '')
const sourceId = computed(() => {
  const version = props.file?.content_hash || props.file?.etag || `${props.file?.size || 0}:${props.file?.updated_at || props.file?.time || ''}`
  return `document:${props.userId}:${driveId.value}:${props.file?.file_id || ''}:${version}`
})
const usesSemanticEmbeddings = computed(() => canUseSemanticEmbeddings(createBookAISettings().provider))
const privacyText = computed(() => usesSemanticEmbeddings.value
  ? '文档会在本机解析。建立语义索引时，文本分块会发送至内置 AI 或本地 Ollama；回答时最多发送 5 个相关片段。'
  : '当前为 BYOK 模式：文档全文、索引和关键词检索均在本机完成，不会调用 embedding API。回答时最多发送 5 个相关片段给你配置的模型。')

function close() {
  abortController.value?.abort()
  emit('update:visible', false)
}

async function buildIndex() {
  if (!props.file || indexing.value) return
  const config = getAIConfig()
  if (!config) { message.warning('请先在设置中配置 AI 模型'); return }
  indexing.value = true
  indexed.value = false
  answer.value = ''
  citations.value = []
  try {
    status.value = '正在获取文档...'
    const download = await AliFile.ApiFileDownloadUrl(props.userId, driveId.value, props.file.file_id, 14_400)
    if (typeof download === 'string') throw new Error(download)
    const response = await fetch(download.url, { headers: download.headers || {} })
    if (!response.ok) throw new Error(`下载文档失败: HTTP ${response.status}`)
    const data = await response.arrayBuffer()
    const settings = createBookAISettings()
    const provider = getAIProvider(settings)
    await indexDocumentLocally({
      sourceId: sourceId.value,
      fileName: fileName.value,
      data,
      embeddingModel: canUseSemanticEmbeddings(settings.provider) ? provider.getEmbeddingModel() : undefined,
      onProgress: progress => {
        const labels = { parsing: '正在本机解析', chunking: '正在本机分块', embedding: usesSemanticEmbeddings.value ? '正在生成语义索引' : '正在建立本地关键词索引', saving: '正在保存本地索引' }
        status.value = progress.detail || `${labels[progress.phase]}${progress.total > 1 ? ` ${progress.current}/${progress.total}` : ''}`
      }
    })
    indexed.value = true
    status.value = '索引已保存在本机，可以开始提问'
  } catch (error: any) {
    status.value = errorMessage(error)
    message.error(status.value)
  } finally {
    indexing.value = false
  }
}

async function ask() {
  const prompt = question.value.trim()
  if (!prompt || !indexed.value || asking.value) return
  const config = getAIConfig()
  if (!config) { message.warning('请先在设置中配置 AI 模型'); return }
  const settings = createBookAISettings()
  const provider = getAIProvider(settings)
  abortController.value = new AbortController()
  asking.value = true
  answer.value = ''
  citations.value = []
  status.value = '正在检索本地文档...'
  try {
    await askIndexedDocument({
      sourceId: sourceId.value,
      fileName: fileName.value,
      question: prompt,
      model: config,
      embeddingModel: canUseSemanticEmbeddings(settings.provider) ? provider.getEmbeddingModel() : undefined,
      signal: abortController.value.signal,
      onToken: token => { status.value = ''; answer.value += token },
      onCitation: citation => {
        const item = { location: citation.location || (citation.page ? `页 ${citation.page}` : '正文'), section: citation.section || '正文', text: citation.text }
        if (!citations.value.some(existing => existing.location === item.location && existing.text === item.text)) citations.value.push(item)
      }
    })
  } catch (error: any) {
    if (error?.name !== 'AbortError') status.value = error?.message || '文档问答失败'
  } finally {
    asking.value = false
    abortController.value = null
  }
}

watch(() => props.visible, visible => {
  if (visible) {
    indexed.value = false
    question.value = ''
    answer.value = ''
    citations.value = []
    status.value = privacyText.value
    void buildIndex()
  }
})
</script>

<template>
  <a-modal :footer='false' :visible='visible' :width='760' unmount-on-close @cancel='close'>
    <template #title>用 AI 分析 · {{ fileName }}</template>
    <div class='document-ai-privacy'>{{ privacyText }}</div>
    <a-progress v-if='indexing' :percent='0.6' status='normal' />
    <div class='document-ai-status'>{{ status }}</div>
    <div v-if='indexed' class='document-ai-question'>
      <a-textarea v-model='question' :auto-size='{ minRows: 2, maxRows: 5 }' placeholder='询问这份文档的内容、摘要或要点' @keydown.meta.enter.prevent='ask' @keydown.ctrl.enter.prevent='ask' />
      <a-button :loading='asking' type='primary' @click='ask'>提问</a-button>
    </div>
    <div v-if='answer' class='document-ai-answer'>{{ answer }}</div>
    <div v-if='citations.length' class='document-ai-citations'>
      <div class='document-ai-citations-title'>引用</div>
      <div v-for='citation in citations' :key='citation.location + citation.text' class='document-ai-citation'>
        <strong>{{ citation.section }} · {{ citation.location }}</strong>
        <p>{{ citation.text }}</p>
      </div>
    </div>
  </a-modal>
</template>

<style scoped>
.document-ai-privacy { margin-bottom: 12px; padding: 10px 12px; border-radius: 8px; color: var(--color-text-2); background: var(--color-fill-2); }
.document-ai-status { min-height: 24px; margin: 8px 0 12px; color: var(--color-text-3); }
.document-ai-question { display: flex; align-items: flex-end; gap: 10px; }
.document-ai-answer { margin-top: 18px; padding: 16px; border-radius: 10px; white-space: pre-wrap; line-height: 1.7; background: var(--color-fill-1); }
.document-ai-citations { margin-top: 16px; }
.document-ai-citations-title { margin-bottom: 8px; font-weight: 600; }
.document-ai-citation { margin-bottom: 8px; padding: 10px 12px; border: 1px solid var(--color-border-2); border-radius: 8px; }
.document-ai-citation p { margin: 6px 0 0; color: var(--color-text-2); line-height: 1.55; }
</style>
