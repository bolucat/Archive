<script setup lang="ts">
import { ref } from 'vue'
import { Files, Loader2, AlertCircle, ChevronRight } from 'lucide-vue-next'
import type { FileResult } from './types'

defineProps<{
  state: 'scanning' | 'done' | 'error'
  output?: { totalFiles: number; groups: { name: string; size: number; files: FileResult[] }[] }
  error?: string
}>()

const emit = defineEmits<{ (e: 'select', files: FileResult[]): void; (e: 'navigate', file: FileResult): void; (e: 'retry'): void }>()
const expanded = ref<Set<number>>(new Set())
const selected = ref<Set<string>>(new Set())

function toggleGroup(i: number) {
  const next = new Set(expanded.value)
  next.has(i) ? next.delete(i) : next.add(i)
  expanded.value = next
}

function toggleFile(fileId: string) {
  const next = new Set(selected.value)
  next.has(fileId) ? next.delete(fileId) : next.add(fileId)
  selected.value = next
}

function toggleAllInGroup(group: { files: FileResult[] }) {
  const ids = group.files.map(f => f.fileId)
  const allSelected = ids.every(id => selected.value.has(id))
  const next = new Set(selected.value)
  ids.forEach(id => allSelected ? next.delete(id) : next.add(id))
  selected.value = next
}

function handleDeleteSelected(group: { files: FileResult[] }) {
  const files = group.files.filter(f => selected.value.has(f.fileId))
  if (files.length) emit('select', files)
}

function formatSize(bytes: number): string {
  if (!bytes) return ''
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB'
  return (bytes / 1024 / 1024 / 1024).toFixed(2) + ' GB'
}
</script>

<template>
  <div class="dc-card">
    <div v-if="state === 'scanning'" class="dc-status">
      <Loader2 :size="14" :stroke-width="2" class="dc-spin" />
      <span>正在扫描重复文件...</span>
    </div>

    <template v-else-if="state === 'done' && output">
      <div class="dc-header">
        <Files :size="14" :stroke-width="1.5" />
        <span>扫描 {{ output.totalFiles }} 个文件，发现 {{ output.groups?.length || 0 }} 组重复</span>
      </div>
      <div v-if="output.groups?.length" class="dc-groups">
        <div v-for="(g, gi) in output.groups" :key="gi" class="dc-group">
          <button class="dc-group-header" @click="toggleGroup(gi)">
            <ChevronRight :size="12" :stroke-width="2" class="dc-chevron" :class="{ open: expanded.has(gi) }" />
            <span class="dc-group-name">{{ g.name }}</span>
            <span class="dc-group-size">{{ formatSize(g.size) }}</span>
            <span class="dc-group-count">{{ g.files.length }} 份</span>
          </button>
          <div v-show="expanded.has(gi)" class="dc-group-files">
            <div class="dc-group-actions">
              <button class="dc-select-all" @click="toggleAllInGroup(g)">全选/取消</button>
              <button
                class="dc-delete-selected"
                :disabled="!g.files.some(f => selected.has(f.fileId))"
                @click="handleDeleteSelected(g)"
              >删除选中</button>
            </div>
            <div v-for="f in g.files" :key="f.fileId" class="dc-file" :class="{ selected: selected.has(f.fileId) }">
              <input type="checkbox" :checked="selected.has(f.fileId)" @change="toggleFile(f.fileId)" />
              <span class="dc-file-name" @click="emit('navigate', f)">{{ f.name }}</span>
              <span class="dc-file-loc">{{ f.providerName }}{{ f.isDir ? ' · 目录' : '' }}</span>
            </div>
          </div>
        </div>
      </div>
      <div v-else class="dc-empty">未发现重复文件</div>
    </template>

    <div v-else-if="state === 'error'" class="dc-status dc-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>{{ error }}</span>
      <button class="dc-retry" @click="emit('retry')">重试</button>
    </div>
  </div>
</template>

<style scoped>
.dc-card { margin: 12px 0; border-radius: 13px; background: color-mix(in srgb, var(--color-fill-1) 78%, transparent); border: 1px solid color-mix(in srgb, var(--color-border-2) 88%, transparent); overflow: hidden; box-shadow: 0 10px 26px rgba(0, 0, 0, .06); }
.dc-status { display: flex; align-items: center; gap: 8px; padding: 12px 14px; font-size: 13px; color: var(--color-text-3); }
.dc-error { color: rgb(var(--danger-6)); }
.dc-header { display: flex; align-items: center; gap: 7px; padding: 11px 14px; font-size: 12px; font-weight: 650; color: #f4b84f; border-bottom: 1px solid var(--color-border-2); background: rgba(244, 184, 79, .07); }
.dc-groups { padding: 4px 0; }
.dc-group { border-bottom: 1px solid var(--color-border-2); }
.dc-group:last-child { border-bottom: none; }
.dc-group-header { display: flex; align-items: center; gap: 6px; width: 100%; padding: 8px 14px; border: 0; background: transparent; cursor: pointer; font-family: inherit; color: inherit; text-align: left; }
.dc-group-header:hover { background: var(--color-fill-2); }
.dc-chevron { flex-shrink: 0; transition: transform 0.15s; }
.dc-chevron.open { transform: rotate(90deg); }
.dc-group-name { flex: 1; font-size: 13px; font-weight: 500; color: var(--color-text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.dc-group-size { font-size: 11px; color: var(--color-text-4); }
.dc-group-count { font-size: 11px; color: rgb(var(--warning-6)); font-weight: 600; }
.dc-group-files { padding: 0 14px 8px; }
.dc-group-actions { display: flex; gap: 8px; padding: 4px 0 8px 20px; }
.dc-select-all, .dc-delete-selected { font-size: 11px; color: var(--color-text-4); background: transparent; border: 0; cursor: pointer; padding: 1px 4px; }
.dc-delete-selected:not(:disabled) { color: rgb(var(--danger-6)); }
.dc-delete-selected:disabled { opacity: 0.4; cursor: default; }
.dc-file { display: flex; align-items: center; gap: 8px; padding: 4px 0 4px 20px; }
.dc-file.selected { background: rgba(var(--primary-6), 0.04); border-radius: 4px; }
.dc-file input[type="checkbox"] { margin: 0; }
.dc-file-name { flex: 1; font-size: 12px; color: var(--color-text-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; cursor: pointer; }
.dc-file-name:hover { color: rgb(var(--primary-6)); }
.dc-file-loc { font-size: 10px; color: var(--color-text-4); flex-shrink: 0; }
.dc-empty { padding: 16px 14px; font-size: 13px; color: var(--color-text-4); text-align: center; }
.dc-retry { margin-left: auto; padding: 2px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: transparent; border: 1px solid rgb(var(--primary-6)); border-radius: 4px; cursor: pointer; }
.dc-spin { animation: dc-spin 1s linear infinite; }
@keyframes dc-spin { to { transform: rotate(360deg); } }
</style>
