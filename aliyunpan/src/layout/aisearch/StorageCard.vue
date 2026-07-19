<script setup lang="ts">
import { HardDrive, Loader2, AlertCircle, ChevronRight } from 'lucide-vue-next'
import { ref } from 'vue'
import type { FileResult } from './types'
import { humanSize } from '../../utils/format'

defineProps<{
  state: 'scanning' | 'done' | 'error'
  output?: { drives: { name: string; totalSize: number; fileCount: number; topLarge: FileResult[] }[]; oldestFiles: FileResult[]; unusedFiles: FileResult[] }
  error?: string
}>()

const emit = defineEmits<{ (e: 'navigate', file: FileResult): void; (e: 'retry'): void }>()
const showLarge = ref(false)
const showUnused = ref(false)
</script>

<template>
  <div class="sc-card">
    <div v-if="state === 'scanning'" class="sc-status">
      <Loader2 :size="14" :stroke-width="2" class="sc-spin" />
      <span>正在分析存储空间...</span>
    </div>

    <template v-else-if="state === 'done' && output">
      <div class="sc-header"><HardDrive :size="14" :stroke-width="1.5" /> 存储空间分析</div>
      <div class="sc-drives">
        <div v-for="d in output.drives" :key="d.name" class="sc-drive">
          <div class="sc-drive-name">{{ d.name }}</div>
          <div class="sc-drive-bar"><div class="sc-drive-fill" :style="{ width: Math.min(100, (d.totalSize / (1024*1024*1024*1024)) * 100) + '%' }" /></div>
          <div class="sc-drive-stat">{{ humanSize(d.totalSize) }} · {{ d.fileCount }} 个文件</div>
        </div>
      </div>

      <button v-if="output.drives[0]?.topLarge?.length" class="sc-section-btn" @click="showLarge = !showLarge">
        <ChevronRight :size="12" :stroke-width="2" class="sc-chevron" :class="{ open: showLarge }" /> TOP 10 大文件
      </button>
      <div v-show="showLarge" class="sc-file-list">
        <div v-for="(f, i) in (output.drives[0]?.topLarge || []).slice(0, 10)" :key="i" class="sc-file" @click="emit('navigate', f)">
          <span class="sc-file-name">{{ f.name }}</span>
          <span class="sc-file-size">{{ humanSize(f.size) }}</span>
          <span class="sc-file-drive">{{ f.providerName }}</span>
        </div>
      </div>

      <button v-if="output.oldestFiles?.length" class="sc-section-btn" @click="showUnused = !showUnused">
        <ChevronRight :size="12" :stroke-width="2" class="sc-chevron" :class="{ open: showUnused }" /> 最旧文件 ({{ output.oldestFiles.length }})
      </button>
      <div v-show="showUnused" class="sc-file-list">
        <div v-for="(f, i) in (output.oldestFiles || []).slice(0, 10)" :key="i" class="sc-file" @click="emit('navigate', f)">
          <span class="sc-file-name">{{ f.name }}</span>
          <span class="sc-file-size">{{ humanSize(f.size) }}</span>
          <span class="sc-file-drive">{{ f.providerName }}</span>
        </div>
      </div>
    </template>

    <div v-else-if="state === 'error'" class="sc-status sc-error">
      <AlertCircle :size="14" :stroke-width="1.5" /><span>{{ error }}</span>
      <button class="sc-retry" @click="emit('retry')">重试</button>
    </div>
  </div>
</template>

<style scoped>
.sc-card { margin: 12px 0; border-radius: 13px; background: color-mix(in srgb, var(--color-fill-1) 78%, transparent); border: 1px solid color-mix(in srgb, var(--color-border-2) 88%, transparent); overflow: hidden; box-shadow: 0 10px 26px rgba(0, 0, 0, .06); }
.sc-status { display: flex; align-items: center; gap: 8px; padding: 12px 14px; font-size: 13px; color: var(--color-text-3); }
.sc-error { color: rgb(var(--danger-6)); }
.sc-header { display: flex; align-items: center; gap: 7px; padding: 11px 14px; font-size: 12px; font-weight: 650; color: #45c5a3; border-bottom: 1px solid var(--color-border-2); background: rgba(69, 197, 163, .06); }
.sc-drives { padding: 8px 14px; display: flex; flex-direction: column; gap: 10px; }
.sc-drive-name { font-size: 13px; font-weight: 500; color: var(--color-text-2); }
.sc-drive-bar { height: 6px; background: var(--color-fill-2); border-radius: 3px; margin: 4px 0; overflow: hidden; }
.sc-drive-fill { height: 100%; background: linear-gradient(90deg, #36bfa0, rgb(var(--primary-5))); border-radius: 3px; transition: width .3s; }
.sc-drive-stat { font-size: 11px; color: var(--color-text-4); }
.sc-section-btn { display: flex; align-items: center; gap: 6px; width: 100%; padding: 10px 14px; border: 0; border-top: 1px solid var(--color-border-2); background: transparent; cursor: pointer; font-size: 12px; font-weight: 600; color: var(--color-text-3); font-family: inherit; }
.sc-section-btn:hover { background: var(--color-fill-2); }
.sc-chevron { flex-shrink: 0; transition: transform 0.15s; }
.sc-chevron.open { transform: rotate(90deg); }
.sc-file-list { padding: 0 14px 8px; }
.sc-file { display: flex; align-items: center; gap: 8px; padding: 4px 0; cursor: pointer; }
.sc-file:hover .sc-file-name { color: rgb(var(--primary-6)); }
.sc-file-name { flex: 1; font-size: 12px; color: var(--color-text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sc-file-size { font-size: 11px; color: var(--color-text-4); flex-shrink: 0; }
.sc-file-drive { font-size: 10px; color: var(--color-text-4); flex-shrink: 0; }
.sc-retry { margin-left: auto; padding: 2px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: transparent; border: 1px solid rgb(var(--primary-6)); border-radius: 4px; cursor: pointer; }
.sc-spin { animation: sc-spin 1s linear infinite; }
@keyframes sc-spin { to { transform: rotate(360deg); } }
</style>
