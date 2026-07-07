<script setup lang="ts">
import { Search, ExternalLink, Loader2, AlertCircle } from 'lucide-vue-next'
import type { FileResult } from './types'

const props = defineProps<{
  state: 'pending' | 'running' | 'done' | 'error'
  input?: { keyword: string }
  output?: { total: number; files: FileResult[] }
  error?: string
}>()

const emit = defineEmits<{ (e: 'navigate', file: FileResult): void; (e: 'retry'): void }>()

function formatSize(bytes: number): string {
  if (!bytes) return ''
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB'
  return (bytes / 1024 / 1024 / 1024).toFixed(2) + ' GB'
}
</script>

<template>
  <div class="sf-card">
    <!-- running -->
    <div v-if="state === 'running'" class="sf-status">
      <Loader2 :size="14" :stroke-width="2" class="sf-spin" />
      <span>正在搜索云盘 "{{ input?.keyword }}" ...</span>
    </div>

    <!-- done -->
    <template v-else-if="state === 'done' && output">
      <div class="sf-header">
        <Search :size="14" :stroke-width="1.5" />
        <span>搜索 "{{ input?.keyword }}" — 找到 {{ output.total }} 个文件</span>
      </div>
      <div v-if="output.files.length" class="sf-list">
        <button
          v-for="(f, i) in output.files.slice(0, 10)"
          :key="i"
          class="sf-item"
          @click="emit('navigate', f)"
        >
          <span class="sf-item-icon">📄</span>
          <div class="sf-item-body">
            <div class="sf-item-name">{{ f.name }}</div>
            <div class="sf-item-meta">
              {{ formatSize(f.size) }} · {{ f.providerName }} {{ f.isDir ? '· 目录' : '' }}
            </div>
          </div>
          <ExternalLink :size="12" :stroke-width="1.5" class="sf-item-go" />
        </button>
        <div v-if="output.files.length > 10" class="sf-more">
          ...还有 {{ output.files.length - 10 }} 个文件
        </div>
      </div>
      <div v-else class="sf-empty">未找到匹配文件</div>
    </template>

    <!-- error -->
    <div v-else-if="state === 'error'" class="sf-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>搜索失败: {{ error }}</span>
      <button class="sf-retry" @click="emit('retry')">重试</button>
    </div>

    <!-- pending -->
    <div v-else class="sf-status sf-muted">
      <Search :size="14" :stroke-width="1.5" />
      准备搜索云盘...
    </div>
  </div>
</template>

<style scoped>
.sf-card { margin: 8px 0; border-radius: 8px; background: var(--color-fill-1); border: 1px solid var(--color-border-2); overflow: hidden; }
.sf-status { display: flex; align-items: center; gap: 8px; padding: 10px 14px; font-size: 13px; color: var(--color-text-3); }
.sf-muted { color: var(--color-text-4); }
.sf-header { display: flex; align-items: center; gap: 6px; padding: 10px 14px; font-size: 12px; font-weight: 600; color: var(--color-text-3); border-bottom: 1px solid var(--color-border-2); }
.sf-list { display: flex; flex-direction: column; }
.sf-item { display: flex; align-items: center; gap: 10px; width: 100%; padding: 8px 14px; border: 0; background: transparent; text-align: left; cursor: pointer; transition: background 0.1s; color: inherit; font-family: inherit; }
.sf-item:hover { background: var(--color-fill-2); }
.sf-item-icon { font-size: 16px; flex-shrink: 0; }
.sf-item-body { flex: 1; min-width: 0; }
.sf-item-name { font-size: 13px; font-weight: 500; color: var(--color-text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sf-item-meta { font-size: 11px; color: var(--color-text-4); margin-top: 1px; }
.sf-item-go { flex-shrink: 0; color: var(--color-text-4); opacity: 0; transition: opacity 0.1s; }
.sf-item:hover .sf-item-go { opacity: 1; }
.sf-more { padding: 6px 14px; font-size: 12px; color: var(--color-text-4); }
.sf-empty { padding: 12px 14px; font-size: 13px; color: var(--color-text-4); }
.sf-error { display: flex; align-items: center; gap: 8px; padding: 10px 14px; font-size: 13px; color: rgb(var(--danger-6)); }
.sf-retry { margin-left: auto; padding: 2px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: transparent; border: 1px solid rgb(var(--primary-6)); border-radius: 4px; cursor: pointer; }
.sf-retry:hover { background: rgba(var(--primary-6), 0.08); }
.sf-spin { animation: sf-spin 1s linear infinite; }
@keyframes sf-spin { to { transform: rotate(360deg); } }
</style>
