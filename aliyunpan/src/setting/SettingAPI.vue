<script setup lang='ts'>
import { computed, ref, watch } from 'vue'
import useSettingStore from './settingstore'
import IconFont from '../components/IconFont.vue'
import { testAIConnection } from '../utils/bookAI'
import { getAIProvider } from '../services/ai/providers'
import { fetchOpenRouterModels } from '../services/ai/providers/OpenRouterProvider'
import type { OpenRouterModelInfo } from '../services/ai/providers/OpenRouterProvider'
import { GATEWAY_MODELS } from '../services/ai/constants'
import type { AISettings } from '../services/ai/types'
import { DEFAULT_AI_SETTINGS } from '../services/ai/constants'

const settingStore = useSettingStore()

const azureKey = ref(settingStore.apiAzureSpeechKey)
const azureRegion = ref(settingStore.apiAzureSpeechRegion)
const aiEnabled = ref(!!settingStore.apiAIModelProvider && !!settingStore.apiAIModelId)
const aiProvider = ref(settingStore.apiAIModelProvider)
const aiKey = ref(settingStore.apiAIModelKey)
const aiModelId = ref(settingStore.apiAIModelId)
const aiBaseUrl = ref(settingStore.apiAIBaseUrl)
const aiEmbeddingModelId = ref(settingStore.apiAIEmbeddingModelId)
const aiSpoilerProtection = ref(settingStore.apiAISpoilerProtection)
const aiMaxContextChunks = ref(settingStore.apiAIMaxContextChunks)
const aiIndexingMode = ref(settingStore.apiAIIndexingMode)
const aiReedyEnabled = ref(settingStore.apiAIReedyEnabled)
const aiReedyRuntime = ref(settingStore.apiAIReedyRuntime)
const aiFetching = ref(false)
const aiModels = ref<{ id: string; name: string }[]>([])
const aiEmbeddingModels = ref<{ id: string; name: string }[]>([])

const DEFAULT_EMBEDDING_MODELS = [
  { id: 'text-embedding-3-small', name: 'text-embedding-3-small (OpenAI)' },
  { id: 'text-embedding-3-large', name: 'text-embedding-3-large (OpenAI)' },
  { id: 'text-embedding-ada-002', name: 'text-embedding-ada-002 (OpenAI)' },
  { id: 'BAAI/bge-large-zh-v1.5', name: 'bge-large-zh-v1.5 (SiliconFlow)' },
  { id: 'BAAI/bge-m3', name: 'bge-m3 (SiliconFlow)' },
]

const embeddingModelOptions = computed(() => {
  const fetched = aiEmbeddingModels.value.map(m => ({ id: m.id, name: m.name }))
  for (const def of DEFAULT_EMBEDDING_MODELS) {
    if (!fetched.find(f => f.id === def.id)) fetched.push(def)
  }
  return fetched
})
const aiFetchError = ref('')
const aiTestStatus = ref<'idle' | 'testing' | 'success' | 'error'>('idle')
const aiTestMsg = ref('')

watch(azureKey, (v) => settingStore.updateStore({ apiAzureSpeechKey: v }))
watch(azureRegion, (v) => settingStore.updateStore({ apiAzureSpeechRegion: v }))
watch(aiProvider, (v) => {
  settingStore.updateStore({ apiAIModelProvider: v })
  aiModels.value = []; aiEmbeddingModels.value = []; aiFetchError.value = ''
  aiKey.value = ''; aiModelId.value = ''; aiEmbeddingModelId.value = ''
  settingStore.updateStore({ apiAIModelKey: '', apiAIModelId: '', apiAIEmbeddingModelId: '' })
  // Auto-fill base URL from provider default
  const info = PROVIDER_INFO[v]
  if (info?.endpoint) {
    aiBaseUrl.value = info.endpoint
    settingStore.updateStore({ apiAIBaseUrl: info.endpoint })
  } else {
    aiBaseUrl.value = ''
    settingStore.updateStore({ apiAIBaseUrl: '' })
  }
})
watch(aiKey, (v) => settingStore.updateStore({ apiAIModelKey: v }))
watch(aiModelId, (v) => settingStore.updateStore({ apiAIModelId: v }))
watch(aiBaseUrl, (v) => settingStore.updateStore({ apiAIBaseUrl: v }))
watch(aiEmbeddingModelId, (v) => settingStore.updateStore({ apiAIEmbeddingModelId: v }))
watch(aiSpoilerProtection, (v) => settingStore.updateStore({ apiAISpoilerProtection: v }))
watch(aiMaxContextChunks, (v) => settingStore.updateStore({ apiAIMaxContextChunks: v }))
watch(aiIndexingMode, (v) => settingStore.updateStore({ apiAIIndexingMode: v }))
watch(aiReedyEnabled, (v) => settingStore.updateStore({ apiAIReedyEnabled: v }))
watch(aiReedyRuntime, (v) => settingStore.updateStore({ apiAIReedyRuntime: v }))
watch(aiEnabled, (v) => { if (!v) { aiProvider.value = ''; aiKey.value = ''; aiModelId.value = '' } })

const hasAzureConfig = computed(() => !!(azureKey.value && azureRegion.value))
const hasAIConfig = computed(() => !!((aiKey.value || aiProvider.value === 'ollama') && aiModelId.value))

const PROVIDER_INFO: Record<string, { name: string; endpoint: string; fetchable: boolean; getKeyUrl?: string }> = {
  ollama: { name: 'Ollama (Local)', endpoint: 'http://127.0.0.1:11434', fetchable: true },
  'ai-gateway': { name: 'AI Gateway (Cloud)', endpoint: '', fetchable: false, getKeyUrl: 'https://vercel.com/docs/ai/ai-gateway' },
  openai: { name: 'OpenAI', endpoint: 'https://api.openai.com/v1', fetchable: true },
  deepseek: { name: 'DeepSeek', endpoint: 'https://api.deepseek.com/v1', fetchable: true },
  qwen: { name: '通义千问', endpoint: 'https://dashscope.aliyuncs.com/compatible-mode/v1', fetchable: true },
  zhipu: { name: '智谱 AI', endpoint: 'https://open.bigmodel.cn/api/paas/v4', fetchable: true },
  moonshot: { name: '月之暗面', endpoint: 'https://api.moonshot.cn/v1', fetchable: true },
  siliconflow: { name: '硅基流动', endpoint: 'https://api.siliconflow.cn/v1', fetchable: true },
  openrouter: { name: 'OpenRouter', endpoint: 'https://openrouter.ai/api/v1', fetchable: true, getKeyUrl: 'https://openrouter.ai/keys' },
  custom: { name: '自定义 (OpenAI 兼容)', endpoint: '', fetchable: false },
}

const availableProviders = [
  { key: 'ollama', label: 'Ollama (Local)' },
  { key: 'ai-gateway', label: 'AI Gateway (Cloud)' },
  { key: 'openai', label: 'OpenAI' },
  { key: 'deepseek', label: 'DeepSeek' },
  { key: 'qwen', label: '通义千问' },
  { key: 'zhipu', label: '智谱 AI' },
  { key: 'moonshot', label: '月之暗面' },
  { key: 'siliconflow', label: '硅基流动' },
  { key: 'openrouter', label: 'OpenRouter' },
  { key: 'custom', label: '自定义 (OpenAI 兼容)' },
]

const gatewayModelOptions = [
  { id: GATEWAY_MODELS.GEMINI_FLASH_LITE, name: 'Gemini 2.5 Flash Lite (Free)' },
  { id: GATEWAY_MODELS.GPT_5_NANO, name: 'GPT-5 Nano' },
  { id: GATEWAY_MODELS.LLAMA_4_SCOUT, name: 'Llama 4 Scout' },
  { id: GATEWAY_MODELS.GROK_4_FAST, name: 'Grok 4.1 Fast' },
  { id: GATEWAY_MODELS.DEEPSEEK_V3, name: 'DeepSeek V3' },
  { id: GATEWAY_MODELS.QWEN_3_235B, name: 'Qwen 3 235B' },
]

// Auto-fetch models when provider changes and has API key
watch([aiProvider, aiKey, aiBaseUrl], () => {
  if (aiEnabled.value && aiProvider.value && (aiKey.value || aiProvider.value === 'ollama')) {
    fetchModels()
  }
})

const disabledClass = computed(() => !aiEnabled.value ? 'disabled-section' : '')

async function fetchModels() {
  if (!aiProvider.value) return
  if (aiProvider.value !== 'ollama' && aiProvider.value !== 'ai-gateway' && !aiKey.value) return

  // AI Gateway: show built-in model list
  if (aiProvider.value === 'ai-gateway') {
    aiModels.value = gatewayModelOptions
    aiEmbeddingModels.value = [{ id: 'openai/text-embedding-3-small', name: 'text-embedding-3-small' }]
    aiFetchError.value = ''
    return
  }

  const info = PROVIDER_INFO[aiProvider.value]
  if (!info?.fetchable) return
  const endpoint = aiBaseUrl.value || info.endpoint
  if (!endpoint) return
  aiFetching.value = true
  aiModels.value = []
  aiEmbeddingModels.value = []
  aiFetchError.value = ''

  try {
    if (aiProvider.value === 'ollama') {
      // Ollama: GET /api/tags
      const resp = await fetch(`${endpoint.replace(/\/+$/, '')}/api/tags`)
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const data = await resp.json()
      const list = (data?.models || []) as { name: string; model?: string }[]
      aiModels.value = list.map((m: any) => ({ id: m.name || m.model, name: m.name || m.model })).sort((a: any, b: any) => a.id.localeCompare(b.id))
      aiEmbeddingModels.value = aiModels.value.filter((m: any) => m.id.includes('embed') || m.id.includes('nomic'))
    } else if (aiProvider.value === 'openrouter') {
      // OpenRouter: GET /models
      const models = await fetchOpenRouterModels(endpoint, aiKey.value)
      aiModels.value = models.filter((m: any) => !m.id.includes('embed')).map((m: any) => ({ id: m.id, name: m.name || m.id })).slice(0, 50)
      aiEmbeddingModels.value = models.filter((m: any) => m.id.includes('embed')).map((m: any) => ({ id: m.id, name: m.name || m.id })).slice(0, 20)
    } else {
      // All other OpenAI-compatible providers: GET /models
      const headers: Record<string, string> = { Authorization: `Bearer ${aiKey.value}` }
      const resp = await fetch(`${endpoint.replace(/\/+$/, '')}/models`, { headers })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const data = await resp.json()
      const list: Array<{ id: string; name?: string }> = data?.data || data?.models || []
      const all = list.map((m: any) => ({ id: m.id, name: m.name || m.id })).sort((a: any, b: any) => a.id.localeCompare(b.id))
      aiModels.value = all.filter((m: any) => !m.id.includes('embed') && !m.id.includes('moderation') && !m.id.includes('dall')).slice(0, 80)
      aiEmbeddingModels.value = all.filter((m: any) => m.id.includes('embed')).slice(0, 20)
    }

    if (!aiModels.value.length) aiFetchError.value = '未获取到模型列表 — 请检查 API Key 和 Base URL'
  } catch (e: any) {
    aiFetchError.value = e?.message || '获取失败'
  } finally {
    aiFetching.value = false
  }
}

async function runConnectionTest() {
  aiTestStatus.value = 'testing'; aiTestMsg.value = ''
  try {
    const { ok, message } = await testAIConnection()
    aiTestStatus.value = ok ? 'success' : 'error'; aiTestMsg.value = message
  } catch (e: any) {
    aiTestStatus.value = 'error'; aiTestMsg.value = e?.message || '测试失败'
  }
}
</script>

<template>
  <div class='ai-panel space-y-5'>
    <!-- Azure 语音 -->
    <div class='settingcard'>
      <div class='settinghead'>Azure 语音服务</div>
      <div class='settingspace'></div>
      <div class='settingrow'>
        <span style='min-width:80px'>Speech Key</span>
        <a-input-password v-model:model-value='azureKey' placeholder='粘贴 Azure Speech Key' style='width:320px' allow-clear />
      </div>
      <div class='settingspace'></div>
      <div class='settingrow'>
        <span style='min-width:80px'>Region</span>
        <a-select v-model:model-value='azureRegion' style='width:160px' placeholder='选择区域'>
          <a-option value='eastasia'>East Asia</a-option>
          <a-option value='southeastasia'>Southeast Asia</a-option>
          <a-option value='eastus'>East US</a-option>
          <a-option value='westus'>West US</a-option>
          <a-option value='northeurope'>North Europe</a-option>
          <a-option value='westeurope'>West Europe</a-option>
          <a-option value='japaneast'>Japan East</a-option>
          <a-option value='australiaeast'>Australia East</a-option>
        </a-select>
        <span style='color:var(--color-text-3);font-size:12px;margin-left:8px'>{{ hasAzureConfig ? '已配置 · ' + azureRegion : '未配置' }}</span>
      </div>
    </div>

    <!-- 服务商 + API 配置 -->
    <div :class="['settingcard', disabledClass]">
      <div class='settinghead'>AI 助手</div>
      <div class='settingspace'></div>
      <div class='settingrow'>
        <span>启用 AI 助手</span>
        <a-switch v-model:model-value='aiEnabled' />
      </div>
      <div class='settingspace'></div>
      <div class='settinghead'>服务商 &amp; API 配置</div>
      <div class='settingspace'></div>
      <div class='settingrow'>
        <a-select v-model:model-value='aiProvider' style='width:100%' :disabled='!aiEnabled' placeholder='选择服务商'>
          <a-option v-for='prov in availableProviders' :key='prov.key' :value='prov.key'>{{ prov.label }}</a-option>
        </a-select>
      </div>
      <div class='settingspace'></div>

      <template v-if="aiProvider">

      <!-- Ollama Server URL -->
      <div v-if="aiProvider === 'ollama'" class='settingrow'>
        <span>Server URL</span>
        <a-button size='mini' :loading='aiFetching' :disabled='!aiEnabled' @click='fetchModels' title='刷新模型'>
          <template #icon><IconFont name='iconrefresh' /></template>
        </a-button>
      </div>
      <div v-if="aiProvider === 'ollama'" class='settingrow'>
        <a-input v-model:model-value='aiBaseUrl' placeholder='http://127.0.0.1:11434' style='width:100%' :disabled='!aiEnabled' />
        <div v-if="aiProvider === 'ollama'" class='settingspace' />
      </div>

      <!-- API Key (non-ollama) -->
      <template v-if="aiProvider !== 'ollama'">
        <div v-if='PROVIDER_INFO[aiProvider]?.getKeyUrl' class='settingrow'>
          <span>API Key</span>
          <a :href='PROVIDER_INFO[aiProvider]?.getKeyUrl' target='_blank' class='ai-getkey-link'>获取 Key →</a>
        </div>
        <div v-else class='settingrow'><span>API Key</span></div>
        <div class='settingrow'>
          <a-input-password v-model:model-value='aiKey' placeholder='粘贴 API Key' style='width:100%' :disabled='!aiEnabled' allow-clear />
        </div>
        <div class='settingspace' />
      </template>

      <!-- Base URL + refresh for fetchable providers -->
      <div v-if='aiProvider !== "ai-gateway" && aiProvider !== "ollama"' class='settingrow'>
        <span>Base URL</span>
        <a-button v-if='PROVIDER_INFO[aiProvider]?.fetchable' size='mini' :loading='aiFetching' :disabled='!aiEnabled' @click='fetchModels' title='刷新模型列表'>
          <template #icon><IconFont name='iconrefresh' /></template>
        </a-button>
      </div>
      <div v-if='aiProvider !== "ai-gateway" && aiProvider !== "ollama"' class='settingrow'>
        <a-input v-model:model-value='aiBaseUrl' :placeholder='PROVIDER_INFO[aiProvider]?.endpoint || "API 地址"' style='width:100%' :disabled='!aiEnabled' />
      </div>
      <div v-if='aiProvider !== "ai-gateway" && aiProvider !== "ollama"' class='settingspace' />

      <!-- Model selector -->
      <div class='settingrow'>
        <span>AI 模型</span>
        <span v-if='aiFetchError' style='color:rgb(var(--danger-6));font-size:11px'>{{ aiFetchError }}</span>
      </div>
      <div class='settingrow'>
        <a-select
          v-if='aiModels.length'
          v-model:model-value='aiModelId'
          style='width:100%'
          :disabled='!aiEnabled'
          allow-search
          :loading='aiFetching'
        >
          <a-option v-for='m in aiModels' :key='m.id' :value='m.id'>{{ m.name }}</a-option>
        </a-select>
        <a-input v-else v-model:model-value='aiModelId' :placeholder='aiProvider === "ai-gateway" ? "google/gemini-2.5-flash-lite" : "模型 ID"' style='width:100%' :disabled='!aiEnabled' />
      </div>
      <div class='settingspace' />

      <!-- Embedding model -->
      <div class='settingrow'><span>Embedding 模型</span></div>
      <div class='settingrow'>
        <a-select
          v-model:model-value='aiEmbeddingModelId'
          style='width:100%'
          :disabled='!aiEnabled'
          allow-search
        >
          <a-option v-for='m in embeddingModelOptions' :key='m.id' :value='m.id'>{{ m.name }}</a-option>
        </a-select>
      </div>
      <div class='settingrow' style='font-size:11px;color:var(--color-text-3);margin-top:2px'>
        留空则禁用 RAG — 仍可聊天，但无法检索本书内容
      </div>
      <div class='settingspace' />
      <div class='settingrow' style='align-items:center'>
        <a-button size='small' :loading="aiTestStatus === 'testing'" :disabled="aiTestStatus === 'testing' || !hasAIConfig" @click='runConnectionTest'>
          {{ aiTestStatus === 'testing' ? '测试中...' : '测试连接' }}
        </a-button>
        <span v-if="aiTestStatus === 'success'" style='color:rgb(var(--green-6));font-size:13px'>✓ 连接成功</span>
        <span v-if="aiTestStatus === 'error'" style='color:rgb(var(--danger-6));font-size:13px'>{{ aiTestMsg || '连接失败' }}</span>
      </div>
      </template>
    </div>

    <!-- Reedy 检索 -->
    <div :class="['settingcard', disabledClass]">
      <div class='settinghead'>Reedy 检索</div>
      <div class='settingspace'></div>
      <div class='settingrow'>
        <span>启用 Reedy</span>
        <a-switch v-model:model-value='aiReedyEnabled' :disabled='!aiEnabled' />
        <span style='color:var(--color-text-3);font-size:12px;margin-left:8px'>使用 SQLite 混合检索（向量 + 全文），比旧版 IndexedDB 更精准</span>
      </div>
      <div class='settingspace' />
      <div class='settingrow'>
        <span>防剧透</span>
        <a-switch v-model:model-value='aiSpoilerProtection' :disabled='!aiEnabled' />
        <span style='color:var(--color-text-3);font-size:12px;margin-left:8px'>仅基于已读/已索引内容回答，拒绝剧透</span>
      </div>
      <div class='settingspace' />
      <div class='settingrow'>
        <span>上下文段数</span>
        <a-input-number v-model:model-value='aiMaxContextChunks' :min='1' :max='12' style='width:90px' :disabled='!aiEnabled' />
        <span style='margin-left:24px'>索引模式</span>
        <a-select v-model:model-value='aiIndexingMode' style='width:130px' :disabled='!aiEnabled'>
          <a-option value='on-demand'>按需</a-option>
          <a-option value='background'>后台</a-option>
        </a-select>
      </div>
      <div class='settingspace' />
      <div class='settingrow'>
        <span>Agent 运行时</span>
        <a-switch v-model:model-value='aiReedyRuntime' :checked-value="'agent'" :unchecked-value="'mvp'" :disabled='!aiEnabled || !aiReedyEnabled' />
        <span style='color:var(--color-text-3);font-size:12px;margin-left:8px'>{{ aiReedyRuntime === 'agent' ? '多轮 Agent 模式，支持技能和记忆' : '单轮工具调用模式' }}</span>
      </div>
      <div class='settingrow' style='font-size:11px;color:var(--color-text-3);margin-top:2px'>
        Agent 模式需要保存当前索引，切换后需重新索引
      </div>
    </div>

  </div>
</template>

<style scoped>
.disabled-section {
  opacity: .5;
  pointer-events: none;
}
.ai-panel .settingcard {
  background: var(--color-bg-2);
  border: 1px solid var(--color-border-2);
  border-radius: 8px;
  padding: 16px 20px;
  margin-bottom: 12px;
}
.ai-panel .settinghead {
  font-weight: 700;
  font-size: 14px;
  margin-bottom: 8px;
  color: var(--color-text-1);
}
.ai-panel .settingrow {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 4px 0;
}
.ai-getkey-link {
  color: rgb(var(--primary-6));
  font-size: 12px;
  text-decoration: none;
}
.ai-getkey-link:hover { text-decoration: underline }
</style>
