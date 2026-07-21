<script setup lang='ts'>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import useSettingStore from './settingstore'
import IconFont from '../components/IconFont.vue'
import { migrateSoleSavedBYOKAsDefault, testAIConnection } from '../utils/bookAI'
import message from '../utils/message'
import { fetchOpenRouterModels } from '../services/ai/providers/OpenRouterProvider'
import type { OpenRouterModelInfo } from '../services/ai/providers/OpenRouterProvider'
import { GATEWAY_MODELS } from '../services/ai/constants'
import { isPro } from '../utils/usageLimit'
import { openExternal } from '../utils/electronhelper'
import { BOXPLAYER_SITE_URL } from '../utils/boxplayerAuth'

const PRICING_URL = `${BOXPLAYER_SITE_URL}/pricing/`

const settingStore = useSettingStore()
migrateSoleSavedBYOKAsDefault()
const isLoggedIn = ref(false)
try { isLoggedIn.value = localStorage.getItem('app_user_authed') === '1' } catch {}

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
const mediaAcquisitionPreferredQuality = ref(settingStore.mediaAcquisitionPreferredQuality)
const mediaAcquisitionFetchSubtitles = ref(settingStore.mediaAcquisitionFetchSubtitles)
const mediaAcquisitionSubtitleLanguage = ref(settingStore.mediaAcquisitionSubtitleLanguage)
const mediaAcquisitionAssrtEnabled = ref(settingStore.mediaAcquisitionAssrtEnabled)
const mediaAcquisitionAssrtToken = ref(settingStore.mediaAcquisitionAssrtToken)
const mediaAcquisitionPatrolTimes = ref(settingStore.mediaAcquisitionPatrolTimes)
const mediaAcquisitionAutoScanHistorical = ref(settingStore.mediaAcquisitionAutoScanHistorical)
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
const accountIsPro = ref(isPro())
const showAddForm = ref(false)

interface ProviderConfig { apiKey: string; modelId: string; embeddingModelId: string; baseUrl: string }

function loadProviderConfig(provider: string): ProviderConfig {
  try {
    const raw = localStorage.getItem(`ai_provider_config_${provider}`)
    if (raw) return JSON.parse(raw) as ProviderConfig
  } catch {}
  return { apiKey: '', modelId: '', embeddingModelId: '', baseUrl: '' }
}

function saveProviderConfig(provider: string, fields: Partial<ProviderConfig>) {
  try {
    const current = loadProviderConfig(provider)
    const updated = { ...current, ...fields }
    localStorage.setItem(`ai_provider_config_${provider}`, JSON.stringify(updated))
  } catch {}
}

const savedProviderConfigs = computed(() => {
  const result: { provider: string; label: string; modelId: string; apiKey: string; baseUrl: string; isDefault: boolean; isBuiltIn?: boolean; locked?: boolean }[] = []
  const currentDefault = settingStore.apiAIModelProvider
  // 内置 AI — 始终显示，非 Pro 用户锁定
  result.push({
    provider: 'boxplayer-cloud',
    label: '内置 AI',
    modelId: boxPlayerCloudModelOptions[0].id,
    apiKey: '',
    baseUrl: '',
    isDefault: currentDefault === 'boxplayer-cloud' && accountIsPro.value,
    isBuiltIn: true,
    locked: !accountIsPro.value,
  })
  // BYOK 已保存的配置
  for (const prov of availableProviders) {
    if (prov.key === 'boxplayer-cloud') continue
    const cfg = loadProviderConfig(prov.key)
    if (cfg.apiKey || cfg.modelId) {
      result.push({ provider: prov.key, label: prov.label, modelId: cfg.modelId, apiKey: cfg.apiKey, baseUrl: cfg.baseUrl, isDefault: prov.key === currentDefault })
    }
  }
  return result
})

const revealedKeys = ref<Set<string>>(new Set())
function toggleRevealKey(provider: string) {
  if (revealedKeys.value.has(provider)) {
    revealedKeys.value.delete(provider)
  } else {
    revealedKeys.value.add(provider)
  }
  revealedKeys.value = new Set(revealedKeys.value)
}
function maskKey(key: string): string {
  if (!key) return ''
  if (key.length <= 8) return '•'.repeat(key.length)
  return key.slice(0, 4) + '•'.repeat(Math.min(key.length - 8, 20)) + key.slice(-4)
}

function saveCurrentConfig() {
  const v = aiProvider.value
  if (!v) { message.warning('请先选择服务商'); return }
  if (v !== 'ollama' && !aiKey.value) { message.warning('请填写 API Key'); return }
  if (!aiModelId.value) { message.warning('请填写或选择模型 ID'); return }
  saveProviderConfig(v, { apiKey: aiKey.value, modelId: aiModelId.value, embeddingModelId: aiEmbeddingModelId.value, baseUrl: aiBaseUrl.value })
  const shouldSetAsDefault = !settingStore.apiAIModelProvider
  if (shouldSetAsDefault) setAsDefault(v)
  showAddForm.value = false
  aiProvider.value = ''
  if (!shouldSetAsDefault) message.success(`${PROVIDER_INFO[v]?.name || v} 配置已保存`)
}

function deleteProviderConfig(provider: string) {
  try {
    localStorage.removeItem(`ai_provider_config_${provider}`)
    if (settingStore.apiAIModelProvider === provider) {
      settingStore.updateStore({ apiAIModelProvider: '', apiAIModelKey: '', apiAIModelId: '', apiAIEmbeddingModelId: '', apiAIBaseUrl: '' })
    }
    message.success(`${PROVIDER_INFO[provider]?.name || provider} 配置已删除`)
  } catch {}
}

function setAsDefault(provider: string) {
  if (provider === 'boxplayer-cloud') {
    if (!accountIsPro.value) { message.warning('内置 AI 需购买 Pro 后使用'); return }
    settingStore.updateStore({ apiAIModelProvider: 'boxplayer-cloud', apiAIModelKey: '', apiAIModelId: boxPlayerCloudModelOptions[0].id, apiAIEmbeddingModelId: '', apiAIBaseUrl: '' })
    message.success('内置 AI 已设为默认')
    return
  }
  const cfg = loadProviderConfig(provider)
  settingStore.updateStore({
    apiAIModelProvider: provider,
    apiAIModelKey: cfg.apiKey,
    apiAIModelId: cfg.modelId,
    apiAIEmbeddingModelId: cfg.embeddingModelId,
    apiAIBaseUrl: cfg.baseUrl || PROVIDER_INFO[provider]?.endpoint || '',
  })
  message.success(`${PROVIDER_INFO[provider]?.name || provider} 已设为默认`)
}

async function handleUpgradeToPro() {
  if (!isLoggedIn.value) {
    try { localStorage.setItem('boxplayer_show_pricing', '1') } catch {}
  }
  try {
    openExternal(PRICING_URL)
  } catch (e: any) { message.error(e?.message || '打开官网购买页面失败') }
}

function editProvider(provider: string) {
  aiProvider.value = provider
  showAddForm.value = true
}

function refreshProState() {
  accountIsPro.value = isPro()
  isLoggedIn.value = localStorage.getItem('app_user_authed') === '1'
  if (!accountIsPro.value && settingStore.apiAIModelProvider === 'boxplayer-cloud') {
    settingStore.updateStore({ apiAIModelProvider: '', apiAIModelKey: '', apiAIModelId: '', apiAIEmbeddingModelId: '', apiAIBaseUrl: '' })
  }
}

onMounted(() => {
  refreshProState()
  window.addEventListener('storage', refreshProState)
  window.addEventListener('focus', refreshProState)
})

onBeforeUnmount(() => {
  window.removeEventListener('storage', refreshProState)
  window.removeEventListener('focus', refreshProState)
})

watch(aiProvider, (v) => {
  aiModels.value = []; aiEmbeddingModels.value = []; aiFetchError.value = ''
  const cached = loadProviderConfig(v)
  aiKey.value = cached.apiKey
  aiModelId.value = cached.modelId
  aiEmbeddingModelId.value = cached.embeddingModelId
  const info = PROVIDER_INFO[v]
  aiBaseUrl.value = cached.baseUrl || info?.endpoint || ''
})
watch(aiSpoilerProtection, (v) => settingStore.updateStore({ apiAISpoilerProtection: v }))
watch(aiMaxContextChunks, (v) => settingStore.updateStore({ apiAIMaxContextChunks: v }))
watch(aiIndexingMode, (v) => settingStore.updateStore({ apiAIIndexingMode: v }))
watch(aiReedyEnabled, (v) => settingStore.updateStore({ apiAIReedyEnabled: v }))
watch(aiReedyRuntime, (v) => settingStore.updateStore({ apiAIReedyRuntime: v }))
watch(mediaAcquisitionPreferredQuality, (v) => settingStore.updateStore({ mediaAcquisitionPreferredQuality: v }))
watch(mediaAcquisitionFetchSubtitles, (v) => settingStore.updateStore({ mediaAcquisitionFetchSubtitles: v }))
watch(mediaAcquisitionSubtitleLanguage, (v) => settingStore.updateStore({ mediaAcquisitionSubtitleLanguage: v }))
watch(mediaAcquisitionAssrtEnabled, (v) => settingStore.updateStore({ mediaAcquisitionAssrtEnabled: v }))
watch(mediaAcquisitionAssrtToken, (v) => settingStore.updateStore({ mediaAcquisitionAssrtToken: v }))
watch(mediaAcquisitionPatrolTimes, (v) => settingStore.updateStore({ mediaAcquisitionPatrolTimes: v }))
watch(mediaAcquisitionAutoScanHistorical, (v) => settingStore.updateStore({ mediaAcquisitionAutoScanHistorical: v }))

const hasAIConfig = computed(() => !!((aiKey.value || aiProvider.value === 'ollama' || aiProvider.value === 'ai-gateway') && aiModelId.value))

const PROVIDER_INFO: Record<string, { name: string; endpoint: string; fetchable: boolean; getKeyUrl?: string }> = {
  'boxplayer-cloud': { name: '内置 AI', endpoint: '', fetchable: false },
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
  { key: 'boxplayer-cloud', label: '内置 AI' },
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

// 下拉框中只显示 BYOK 服务商，内置 AI 通过已保存配置区域管理
const configurableProviders = computed(() => availableProviders.filter(p => p.key !== 'boxplayer-cloud'))

const boxPlayerCloudModelOptions = [
  { id: 'deepseek/deepseek-v4-pro', name: 'DeepSeek V4 Pro (内置)' },
]

const gatewayModelOptions = [
  { id: GATEWAY_MODELS.GEMINI_FLASH_LITE, name: 'Gemini 2.5 Flash Lite (Free)' },
  { id: GATEWAY_MODELS.GPT_5_NANO, name: 'GPT-5 Nano' },
  { id: GATEWAY_MODELS.LLAMA_4_SCOUT, name: 'Llama 4 Scout' },
  { id: GATEWAY_MODELS.GROK_4_FAST, name: 'Grok 4.1 Fast' },
  { id: GATEWAY_MODELS.DEEPSEEK_V3, name: 'DeepSeek V3' },
  { id: GATEWAY_MODELS.QWEN_3_235B, name: 'Qwen 3 235B' },
]

watch([aiProvider, aiKey, aiBaseUrl], () => {
  if (aiProvider.value && (aiKey.value || aiProvider.value === 'ollama')) {
    fetchModels()
  }
})

async function fetchModels() {
  if (!aiProvider.value) return
  if (aiProvider.value !== 'ollama' && aiProvider.value !== 'ai-gateway' && !aiKey.value) return

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
      const resp = await fetch(`${endpoint.replace(/\/+$/, '')}/api/tags`)
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const data = await resp.json()
      const list = (data?.models || []) as { name: string; model?: string }[]
      aiModels.value = list.map((m: any) => ({ id: m.name || m.model, name: m.name || m.model })).sort((a: any, b: any) => a.id.localeCompare(b.id))
      aiEmbeddingModels.value = aiModels.value.filter((m: any) => m.id.includes('embed') || m.id.includes('nomic'))
    } else if (aiProvider.value === 'openrouter') {
      const models = await fetchOpenRouterModels(endpoint, aiKey.value)
      aiModels.value = models.filter((m: any) => !m.id.includes('embed')).map((m: any) => ({ id: m.id, name: m.name || m.id })).slice(0, 50)
      aiEmbeddingModels.value = models.filter((m: any) => m.id.includes('embed')).map((m: any) => ({ id: m.id, name: m.name || m.id })).slice(0, 20)
    } else {
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
  <!-- AI 助手 -->
  <div class='settingcard'>
    <div class='settinghead'>AI 助手</div>

    <!-- Empty state -->
    <div v-if="!savedProviderConfigs.length" class='ai-empty'>
      <div class="ai-empty-icon">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M12 2a4 4 0 0 1 4 4v1h2a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2h2V6a4 4 0 0 1 4-4z"/><circle cx="12" cy="13" r="1.5"/></svg>
      </div>
      <div class="ai-empty-title">尚未配置 AI 模型</div>
      <div class="ai-empty-desc">{{ accountIsPro ? '点击下方按钮添加 BYOK 服务商，或在已配置列表中将内置 AI 设为默认。' : '登录后可使用 BYOK 服务商；Pro 用户还可使用内置 AI 模型。' }}</div>
      <a-button type='primary' size='small' @click='showAddForm = true'>配置新模型</a-button>
    </div>

    <!-- Has configs -->
    <template v-else>
      <!-- 当前默认 -->
      <div class='ai-section-label'>当前默认</div>
      <div v-for='cfg in savedProviderConfigs.filter(c => c.isDefault)' :key='cfg.provider' class='ai-provider-card active'>
        <div class='ai-provider-main'>
          <div class='ai-provider-info'>
            <span class='ai-provider-name'>{{ cfg.label }}</span>
            <span v-if='!cfg.isBuiltIn' class='ai-provider-model'>{{ cfg.modelId }}</span>
          </div>
          <div class='ai-provider-meta'>
            <span v-if='cfg.isBuiltIn' class='ai-badge-builtin'>Pro 内置</span>
            <span v-else-if='cfg.apiKey' class='ai-badge-key'>已配置 Key</span>
            <span class='ai-badge-active'>当前默认</span>
          </div>
        </div>
      </div>
      <div v-if="!savedProviderConfigs.some(c => c.isDefault)" class='ai-no-default'>
        未设置默认模型，请在下方列表中选择一个设为默认
      </div>

      <div class='ai-section-label'>已配置的模型</div>
      <div class='ai-provider-list'>
        <div v-for='cfg in savedProviderConfigs.filter(c => !c.isDefault)' :key='cfg.provider' class='ai-provider-card' :class='{ locked: cfg.locked }'>
          <div class='ai-provider-main'>
            <div class='ai-provider-info'>
              <span class='ai-provider-name'>{{ cfg.label }}</span>
              <span v-if='!cfg.isBuiltIn' class='ai-provider-model'>{{ cfg.modelId }}</span>
              <span v-if='cfg.isBuiltIn && !cfg.locked' class='ai-badge-builtin'>Pro 内置</span>
              <span v-if='cfg.locked' class='ai-badge-locked'>Pro 专属</span>
            </div>
            <div class='ai-provider-actions'>
              <template v-if='!cfg.isBuiltIn && cfg.apiKey'>
                <span class='ai-key-preview'>{{ revealedKeys.has(cfg.provider) ? cfg.apiKey : maskKey(cfg.apiKey) }}</span>
                <button class='ai-btn-ghost' @click='toggleRevealKey(cfg.provider)'>{{ revealedKeys.has(cfg.provider) ? '隐藏' : '显示' }}</button>
              </template>
              <button v-if='!cfg.locked' class='ai-btn-ghost' @click='setAsDefault(cfg.provider)'>设为默认</button>
              <button v-else class='ai-btn-ghost ai-btn-upgrade' @click='handleUpgradeToPro'>升级到 Pro</button>
              <button v-if='!cfg.isBuiltIn' class='ai-btn-ghost' @click='editProvider(cfg.provider)'>编辑</button>
              <button v-if='!cfg.isBuiltIn' class='ai-btn-delete' @click='deleteProviderConfig(cfg.provider)' title='删除此配置'>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            </div>
          </div>
        </div>
      </div>

      <a-button v-if='!showAddForm' type='outline' size='small' class='ai-add-btn' @click='showAddForm = true'>
        + 配置新模型
      </a-button>
    </template>

    <!-- 配置新模型 form -->
    <div v-if='showAddForm' class='ai-add-form'>
      <div class='ai-add-form-head'>
        <span class='ai-subhead'>配置新模型</span>
        <button class='ai-btn-ghost' @click='showAddForm = false; aiProvider = ""; aiModels = []; aiFetchError = ""'>收起</button>
      </div>
      <div class='settingrow'>
        <a-select v-model:model-value='aiProvider' style='width:100%' placeholder='选择 BYOK 服务商'>
          <a-option v-for='prov in configurableProviders' :key='prov.key' :value='prov.key'>
            {{ prov.label }}
          </a-option>
        </a-select>
      </div>

      <template v-if="aiProvider">
      <div v-if="aiProvider === 'ollama'" class='settingrow'>
        <span class='ai-field-label'>Server URL</span>
        <div class='ai-field-control'>
          <div class='ai-input-row'>
            <a-input v-model:model-value='aiBaseUrl' placeholder='http://127.0.0.1:11434' />
            <a-button size='mini' :loading='aiFetching' @click='fetchModels'><template #icon><IconFont name='iconrefresh' /></template></a-button>
          </div>
        </div>
      </div>

      <template v-if="aiProvider !== 'ollama'">
        <div class='settingrow'>
          <span class='ai-field-label'>API Key</span>
          <div class='ai-field-control'>
            <div class='ai-input-row'>
              <a-input-password v-model:model-value='aiKey' placeholder='粘贴 API Key' allow-clear />
              <a v-if='PROVIDER_INFO[aiProvider]?.getKeyUrl' :href='PROVIDER_INFO[aiProvider]?.getKeyUrl' target='_blank' class='ai-getkey-link'>获取 Key</a>
            </div>
          </div>
        </div>
      </template>

      <template v-if="aiProvider !== 'ai-gateway' && aiProvider !== 'ollama'">
        <div class='settingrow'>
          <span class='ai-field-label'>Base URL</span>
          <div class='ai-field-control'>
            <div class='ai-input-row'>
              <a-input v-model:model-value='aiBaseUrl' :placeholder='PROVIDER_INFO[aiProvider]?.endpoint || "API 地址"' />
              <a-button v-if='PROVIDER_INFO[aiProvider]?.fetchable' size='mini' :loading='aiFetching' @click='fetchModels'><template #icon><IconFont name='iconrefresh' /></template></a-button>
            </div>
          </div>
        </div>
      </template>

      <div class='settingrow'>
        <span class='ai-field-label'>AI 模型</span>
        <div class='ai-field-control'>
          <a-select v-if='aiModels.length' v-model:model-value='aiModelId' allow-search :loading='aiFetching'>
            <a-option v-for='m in aiModels' :key='m.id' :value='m.id'>{{ m.name }}</a-option>
          </a-select>
          <a-input v-else v-model:model-value='aiModelId' :placeholder='aiProvider === "ai-gateway" ? "google/gemini-2.5-flash-lite" : "模型 ID"' />
          <span v-if='aiFetchError' class='ai-field-error'>{{ aiFetchError }}</span>
        </div>
      </div>

      <div class='settingrow'>
        <span class='ai-field-label'>Embedding</span>
        <div class='ai-field-control'>
          <a-select v-model:model-value='aiEmbeddingModelId' allow-search>
            <a-option v-for='m in embeddingModelOptions' :key='m.id' :value='m.id'>{{ m.name }}</a-option>
          </a-select>
        </div>
      </div>
      <div class='ai-field-help'>
        <span class='ai-field-label'></span>
        <span>留空则禁用 RAG — 仍可聊天，但无法检索本书内容</span>
      </div>

      <div class='ai-form-actions'>
        <a-button type='primary' size='small' @click='saveCurrentConfig'>保存配置</a-button>
        <a-button size='small' :loading="aiTestStatus === 'testing'" :disabled="aiTestStatus === 'testing' || !hasAIConfig" @click='runConnectionTest'>
          {{ aiTestStatus === 'testing' ? '测试中...' : '测试连接' }}
        </a-button>
        <span v-if="aiTestStatus === 'success'" class='ai-test-success'>连接成功</span>
        <span v-if="aiTestStatus === 'error'" class='ai-test-error'>{{ aiTestMsg || '连接失败' }}</span>
      </div>
      </template>
    </div>
  </div>

  <!-- Reedy 检索 -->
  <div class='settingcard'>
    <div class='settinghead'>Reedy 检索</div>
    <div class='settingrow'>
      <span class='ai-reedy-label'>启用 Reedy</span>
      <a-switch v-model:model-value='aiReedyEnabled' />
      <span class='ai-hint'>使用 SQLite 混合检索（向量 + 全文），比旧版 IndexedDB 更精准</span>
    </div>
    <div class='settingrow'>
      <span class='ai-reedy-label'>防剧透</span>
      <a-switch v-model:model-value='aiSpoilerProtection' />
      <span class='ai-hint'>仅基于已读/已索引内容回答，拒绝剧透</span>
    </div>
    <div class='settingrow'>
      <span class='ai-reedy-label'>上下文段数</span>
      <a-input-number v-model:model-value='aiMaxContextChunks' :min='1' :max='12' style='width:90px' />
      <span style='margin-left:24px'>索引模式</span>
      <a-select v-model:model-value='aiIndexingMode' style='width:130px'>
        <a-option value='on-demand'>按需</a-option>
        <a-option value='background'>后台</a-option>
      </a-select>
    </div>
    <div class='settingrow'>
      <span class='ai-reedy-label'>Agent 运行时</span>
      <a-switch v-model:model-value='aiReedyRuntime' :checked-value="'agent'" :unchecked-value="'mvp'" :disabled='!aiReedyEnabled' />
      <span class='ai-hint'>{{ aiReedyRuntime === 'agent' ? '多轮 Agent 模式，支持技能和记忆' : '单轮工具调用模式' }}</span>
    </div>
    <div class='ai-footnote'>Agent 模式需要保存当前索引，切换后需重新索引</div>
  </div>

  <div class='settingcard'>
    <div class='settinghead'>媒体获取偏好</div>
    <div class='settingrow'>
      <span class='ai-reedy-label'>资源画质</span>
      <a-select v-model:model-value='mediaAcquisitionPreferredQuality' style='width:150px'>
        <a-option value='auto'>不限（默认）</a-option>
        <a-option value='2160p'>4K（2160P）</a-option>
        <a-option value='1080p'>全高清（1080P）</a-option>
        <a-option value='720p'>高清（720P）</a-option>
        <a-option value='480p'>标清（480P）</a-option>
      </a-select>
      <span class='ai-hint'>作为候选资源优先级；找不到时仍保留其它可导入版本。</span>
    </div>
    <div class='settingrow'>
      <span class='ai-reedy-label'>获取字幕</span>
      <a-switch v-model:model-value='mediaAcquisitionFetchSubtitles' />
      <span class='ai-hint'>优先选择名称中标明字幕语言的资源。</span>
    </div>
    <div class='settingrow' :class='{ disabled: !mediaAcquisitionFetchSubtitles }'>
      <span class='ai-reedy-label'>字幕语言</span>
      <a-select v-model:model-value='mediaAcquisitionSubtitleLanguage' :disabled='!mediaAcquisitionFetchSubtitles' style='width:150px'>
        <a-option value='zh-CN'>中文（默认）</a-option>
        <a-option value='zh-Hant'>繁体中文</a-option>
        <a-option value='en'>英文</a-option>
        <a-option value='ja'>日文</a-option>
        <a-option value='ko'>韩文</a-option>
        <a-option value='auto'>不限</a-option>
      </a-select>
      <span class='ai-hint'>会随每个新建获取任务保存，不影响已经创建的任务。</span>
    </div>
    <div class='settingrow'>
      <span class='ai-reedy-label'>自动中文字幕</span>
      <a-switch v-model:model-value='mediaAcquisitionAssrtEnabled' :disabled='!mediaAcquisitionFetchSubtitles' />
      <span class='ai-hint'>仅非国产内容，且目标网盘支持外链离线时从 ASSRT 补全外挂中文字幕。</span>
    </div>
    <div class='settingrow' :class='{ disabled: !mediaAcquisitionAssrtEnabled || !mediaAcquisitionFetchSubtitles }'>
      <span class='ai-reedy-label'>ASSRT Token</span>
      <a-input-password v-model:model-value='mediaAcquisitionAssrtToken' :disabled='!mediaAcquisitionAssrtEnabled || !mediaAcquisitionFetchSubtitles' allow-clear placeholder='留空使用内置服务；填写后优先使用你的 Token' style='width:320px' />
      <span class='ai-hint'>自带 Token 仅保存于本机，不会发送给 BoxPlayer API。</span>
    </div>
    <div class='settingrow'>
      <span class='ai-reedy-label'>追更巡检时间</span>
      <a-input v-model:model-value='mediaAcquisitionPatrolTimes' placeholder='06:00,21:00' style='width:220px' />
      <span class='ai-hint'>北京时间，逗号分隔，最多 6 个时间点；错过后下次启动会补跑。</span>
    </div>
    <div class='settingrow'>
      <span class='ai-reedy-label'>历史媒体自动补全</span>
      <a-switch v-model:model-value='mediaAcquisitionAutoScanHistorical' />
      <span class='ai-hint'>默认关闭。开启后后台刷新已刮削剧集的播出进度，发现已播缺集时自动创建 Agent 获取任务。</span>
    </div>
  </div>
</template>

<style scoped>
/* ── empty state ── */
.ai-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 28px 20px;
  text-align: center;
}
.ai-empty-icon {
  color: var(--color-text-4);
  opacity: 0.6;
}
.ai-empty-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--color-text-2);
}
.ai-empty-desc {
  max-width: 420px;
  font-size: 12px;
  color: var(--color-text-4);
  line-height: 1.6;
}

/* ── section label ── */
.ai-section-label {
  font-size: 11px;
  font-weight: 700;
  color: var(--color-text-4);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  margin-top: 12px;
}

/* ── provider cards ── */
.ai-provider-card {
  border: 1px solid var(--color-border-2);
  border-radius: 12px;
  margin-top: 6px;
  overflow: hidden;
}
.ai-provider-card.active {
  border-color: rgba(var(--primary-6), 0.35);
  background: rgba(var(--primary-6), 0.05);
}
.ai-provider-card.locked {
  opacity: 0.6;
  border-style: dashed;
}
.ai-provider-card.locked .ai-provider-name,
.ai-provider-card.locked .ai-provider-model {
  color: var(--color-text-4);
}
.ai-provider-main {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  min-height: 46px;
}
.ai-provider-info {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex: 1;
}
.ai-provider-name {
  font-size: 14px;
  font-weight: 700;
  color: var(--color-text-1);
  flex-shrink: 0;
}
.ai-provider-model {
  font-size: 12px;
  color: var(--color-text-3);
  font-family: monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ai-provider-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.ai-provider-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.ai-badge-builtin {
  padding: 2px 8px;
  font-size: 11px;
  font-weight: 700;
  color: #b45309;
  background: rgba(245, 158, 11, 0.12);
  border-radius: 999px;
  line-height: 1.3;
  flex-shrink: 0;
}
.ai-badge-key {
  padding: 2px 8px;
  font-size: 11px;
  font-weight: 600;
  color: var(--color-text-4);
  background: var(--color-fill-2);
  border-radius: 999px;
  line-height: 1.3;
  flex-shrink: 0;
}
.ai-badge-active {
  padding: 2px 8px;
  font-size: 11px;
  font-weight: 700;
  color: rgb(var(--primary-6));
  background: rgba(var(--primary-6), 0.08);
  border-radius: 999px;
  line-height: 1.3;
  flex-shrink: 0;
}
.ai-badge-locked {
  padding: 2px 8px;
  font-size: 11px;
  font-weight: 700;
  color: #b45309;
  background: rgba(245, 158, 11, 0.1);
  border: 1px solid rgba(245, 158, 11, 0.25);
  border-radius: 999px;
  line-height: 1.3;
  flex-shrink: 0;
}
.ai-btn-upgrade {
  color: #b45309 !important;
  background: rgba(245, 158, 11, 0.1) !important;
  font-weight: 700 !important;
}
.ai-btn-upgrade:hover {
  background: rgba(245, 158, 11, 0.2) !important;
}
.ai-key-preview {
  font-family: monospace;
  font-size: 11px;
  color: var(--color-text-4);
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ai-no-default {
  padding: 10px 0;
  font-size: 12px;
  color: var(--color-text-4);
}

/* ── ghost button ── */
.ai-btn-ghost {
  border: none;
  background: var(--color-fill-3);
  color: var(--color-text-3);
  font-size: 12px;
  padding: 3px 10px;
  border-radius: 8px;
  cursor: pointer;
  font-family: inherit;
  flex-shrink: 0;
  line-height: 1.4;
}
.ai-btn-ghost:hover {
  background: var(--color-fill-4);
  color: var(--color-text-1);
}
.ai-btn-delete {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--color-text-4);
  cursor: pointer;
  flex-shrink: 0;
  padding: 0;
}
.ai-btn-delete:hover {
  background: rgba(var(--danger-6), 0.1);
  color: rgb(var(--danger-6));
}

/* ── add button ── */
.ai-add-btn {
  margin-top: 10px;
}

/* ── add form ── */
.ai-add-form {
  margin-top: 14px;
  padding: 16px;
  border: 1px solid var(--color-border-2);
  border-radius: 12px;
  background: var(--color-fill-1);
}
.ai-add-form-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}
.ai-subhead {
  font-size: 13px;
  font-weight: 700;
  color: var(--color-text-2);
}
.ai-form-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 14px;
  padding-top: 10px;
  border-top: 1px solid var(--color-border);
}

/* ── field layout ── */
.ai-field-label {
  display: inline-block;
  flex: 0 0 82px;
  font-size: 13px;
  color: var(--color-text-2);
  line-height: 32px;
  flex-shrink: 0;
}
.ai-field-control {
  flex: 1;
  min-width: 0;
}
.ai-input-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.ai-input-row > :first-child { flex: 1; min-width: 0; }
.ai-field-error {
  display: block;
  margin-top: 4px;
  color: rgb(var(--danger-6));
  font-size: 11px;
  line-height: 1.4;
}
.ai-field-help {
  display: flex;
  align-items: flex-start;
  color: var(--color-text-4);
  font-size: 11px;
  line-height: 1.5;
  margin-top: 4px;
}
.ai-field-help .ai-field-label {
  line-height: inherit;
  padding-top: 0;
}

/* ── reedy ── */
.ai-reedy-label {
  display: inline-block;
  min-width: 88px;
  flex-shrink: 0;
}
.ai-hint {
  color: var(--color-text-3);
  font-size: 12px;
  line-height: 1.5;
}
.ai-footnote {
  color: var(--color-text-4);
  font-size: 11px;
  line-height: 1.5;
  margin-top: 2px;
  padding-left: 96px;
}
.settingrow.disabled { opacity: 0.55; }

/* ── misc ── */
.ai-getkey-link {
  color: rgb(var(--primary-6));
  font-size: 12px;
  text-decoration: none;
  white-space: nowrap;
  flex-shrink: 0;
}
.ai-getkey-link:hover { text-decoration: underline; }
.ai-provider-list { }
.ai-test-success { color: rgb(var(--green-6)); font-size: 13px; font-weight: 600; }
.ai-test-error   { color: rgb(var(--danger-6)); font-size: 13px; }
</style>
