<script setup lang="ts">
import { Globe, Loader2, AlertCircle, Copy, ExternalLink, Download } from 'lucide-vue-next'
import { modalDaoRuShareLink } from '../../utils/modal'
import type { LinkResult } from './types'
import { t } from '../../i18n'

const props = defineProps<{
  state: 'pending' | 'running' | 'done' | 'error'
  input?: { keyword: string }
  output?: { total: number; links: LinkResult[] }
  error?: string
}>()

const emit = defineEmits<{ (e: 'retry'): void }>()

function copyText(text: string) {
  navigator.clipboard.writeText(text).catch(() => {})
}

function canSave(url: string): boolean {
  return /aliyundrive\.com\/s\/|alipan\.com\/s\/|pan\.quark\.cn\/s\//i.test(url)
}

function parseSharePwd(url: string, password: string): string {
  if (password) return password
  const m = url.match(/[?&#]pwd=([0-9a-zA-Z]+)/i) || url.match(/(?:提取码|密码)[^0-9a-zA-Z]{0,8}([0-9a-zA-Z]{4,8})/i)
  return m?.[1] || ''
}

function handleSave(url: string, password: string) {
  modalDaoRuShareLink(url, parseSharePwd(url, password))
}

const PLATFORM_COLORS: Record<string, string> = {
  aliyun: '#7c3aed', quark: '#6366f1', baidu: '#2563eb', '115': '#f59e0b',
  xunlei: '#fbbf24', uc: '#ef4444', tianyi: '#ec4899', '123': '#10b981',
  mobile: '#0ea5e9', pikpak: '#f97316', lanzou: '#06b6d4',
}
</script>

<template>
  <div class="sl-card">
    <!-- running -->
    <div v-if="state === 'running'" class="sl-status">
      <Loader2 :size="14" :stroke-width="2" class="sl-spin" />
      <span>{{ t('ai.card.searchingWeb') }} "{{ input?.keyword }}" ...</span>
    </div>

    <!-- done -->
    <template v-else-if="state === 'done' && output">
      <div class="sl-header">
        <Globe :size="14" :stroke-width="1.5" />
        <span>{{ t('ai.card.webShares') }} — "{{ input?.keyword }}" — {{ output.total }} {{ t('ai.card.links') }}</span>
      </div>
      <div v-if="output.links.length" class="sl-list">
        <div v-for="(l, i) in output.links.slice(0, 10)" :key="i" class="sl-item">
          <span
            class="sl-platform-badge"
            :style="{ background: (PLATFORM_COLORS[l.type] || '#6b7280') }"
          >
            {{ l.type }}
          </span>
          <div class="sl-item-body">
            <div class="sl-item-url">{{ l.url }}</div>
            <div v-if="l.note" class="sl-item-note">{{ l.note }}</div>
          </div>
          <button v-if="canSave(l.url)" class="sl-save-btn" :title="t('ai.card.saveToDrive')" @click="handleSave(l.url, l.password || '')">
            <Download :size="13" :stroke-width="1.5" />
          </button>
          <button class="sl-copy-btn" :title="t('share.copyLink')" @click="copyText(l.url)">
            <Copy :size="13" :stroke-width="1.5" />
          </button>
          <a :href="l.url" target="_blank" class="sl-open-btn" :title="t('common.open')">
            <ExternalLink :size="13" :stroke-width="1.5" />
          </a>
          <span v-if="l.password" class="sl-pass">🔑 {{ l.password }}</span>
        </div>
        <div v-if="output.links.length > 10" class="sl-more">
          ...{{ t('ai.card.more') }} {{ output.links.length - 10 }} {{ t('ai.card.links') }}
        </div>
      </div>
      <div v-else class="sl-empty">{{ t('ai.card.noMatchingLinks') }}</div>
    </template>

    <!-- error -->
    <div v-else-if="state === 'error'" class="sl-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>{{ t('ai.card.searchFailed') }}: {{ error }}</span>
      <button class="sl-retry" @click="emit('retry')">{{ t('common.retry') }}</button>
    </div>

    <!-- pending -->
    <div v-else class="sl-status sl-muted">
      <Globe :size="14" :stroke-width="1.5" />
      {{ t('ai.card.prepareSearchWeb') }}
    </div>
  </div>
</template>

<style scoped>
.sl-card { margin: 12px 0; border-radius: 13px; background: color-mix(in srgb, var(--color-fill-1) 78%, transparent); border: 1px solid color-mix(in srgb, var(--color-border-2) 88%, transparent); overflow: hidden; box-shadow: 0 10px 26px rgba(0, 0, 0, .06); }
.sl-status { display: flex; align-items: center; gap: 8px; padding: 10px 14px; font-size: 13px; color: var(--color-text-3); }
.sl-muted { color: var(--color-text-4); }
.sl-header { display: flex; align-items: center; gap: 7px; padding: 11px 14px; font-size: 12px; font-weight: 650; color: rgb(var(--primary-5)); border-bottom: 1px solid var(--color-border-2); background: rgba(var(--primary-6), .055); }
.sl-list { display: flex; flex-direction: column; }
.sl-item { display: flex; align-items: center; gap: 8px; padding: 9px 14px; }
.sl-platform-badge { flex-shrink: 0; padding: 1px 6px; font-size: 10px; font-weight: 600; color: #fff; border-radius: 3px; text-transform: uppercase; }
.sl-item-body { flex: 1; min-width: 0; }
.sl-item-url { font-size: 12px; color: var(--color-text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: monospace; }
.sl-item-note { font-size: 11px; color: var(--color-text-4); margin-top: 1px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sl-copy-btn, .sl-open-btn, .sl-save-btn { flex-shrink: 0; display: flex; align-items: center; justify-content: center; width: 24px; height: 24px; padding: 0; border: 0; background: transparent; color: var(--color-text-4); cursor: pointer; border-radius: 4px; }
.sl-copy-btn:hover, .sl-open-btn:hover, .sl-save-btn:hover { background: rgba(var(--primary-6), .10); color: var(--color-text-2); }
.sl-save-btn { color: rgb(var(--primary-6)); }
.sl-save-btn:hover { color: rgb(var(--primary-6)); }
.sl-pass { flex-shrink: 0; font-size: 11px; color: var(--color-text-3); }
.sl-more { padding: 6px 14px; font-size: 12px; color: var(--color-text-4); }
.sl-empty { padding: 12px 14px; font-size: 13px; color: var(--color-text-4); }
.sl-error { display: flex; align-items: center; gap: 8px; padding: 10px 14px; font-size: 13px; color: rgb(var(--danger-6)); }
.sl-retry { margin-left: auto; padding: 2px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: transparent; border: 1px solid rgb(var(--primary-6)); border-radius: 4px; cursor: pointer; }
.sl-retry:hover { background: rgba(var(--primary-6), 0.08); }
.sl-spin { animation: sl-spin 1s linear infinite; }
@keyframes sl-spin { to { transform: rotate(360deg); } }
</style>
