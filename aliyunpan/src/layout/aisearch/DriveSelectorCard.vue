<script setup lang="ts">
import { ref } from 'vue'
import { HardDrive, Check } from 'lucide-vue-next'

const props = defineProps<{
  drives: { userId: string; name: string; platform: string; driveId: string }[]
}>()

const emit = defineEmits<{ (e: 'confirm', selected: typeof props.drives): void }>()

const selected = ref<Set<string>>(new Set())

function toggle(userId: string) {
  const next = new Set(selected.value)
  next.has(userId) ? next.delete(userId) : next.add(userId)
  selected.value = next
}

function toggleAll() {
  if (selected.value.size === props.drives.length) {
    selected.value = new Set()
  } else {
    selected.value = new Set(props.drives.map(d => d.userId))
  }
}

function confirm() {
  const picked = props.drives.filter(d => selected.value.has(d.userId))
  if (picked.length) emit('confirm', picked)
}

const PLATFORM_LABELS: Record<string, string> = {
  aliyun: '阿里云盘', quark: '夸克网盘', baidu: '百度网盘', '115': '115网盘',
  '123': '123云盘', tianyi: '天翼云盘', xunlei: '迅雷云盘', pikpak: 'PikPak',
  dropbox: 'Dropbox', onedrive: 'OneDrive', box: 'Box',
}
</script>

<template>
  <div class="ds-card">
    <div class="ds-header">
      <HardDrive :size="14" :stroke-width="1.5" />
      <span>选择要操作的网盘</span>
      <button class="ds-toggle-all" @click="toggleAll">
        {{ selected.size === drives.length ? '取消全选' : '全选' }}
      </button>
    </div>
    <div class="ds-list">
      <button
        v-for="d in drives"
        :key="d.userId"
        class="ds-item"
        :class="{ selected: selected.has(d.userId) }"
        @click="toggle(d.userId)"
      >
        <span class="ds-check">
          <Check v-if="selected.has(d.userId)" :size="12" :stroke-width="3" />
        </span>
        <span class="ds-platform">{{ PLATFORM_LABELS[d.platform] || d.platform }}</span>
        <span class="ds-name">{{ d.name }}</span>
      </button>
    </div>
    <div class="ds-action">
      <button class="ds-confirm" :disabled="!selected.size" @click="confirm">
        确定 ({{ selected.size }})
      </button>
    </div>
  </div>
</template>

<style scoped>
.ds-card { margin: 12px 0; border-radius: 13px; background: linear-gradient(135deg, rgba(var(--primary-6), .10), color-mix(in srgb, var(--color-fill-1) 78%, transparent)); border: 1px solid rgba(var(--primary-6), .24); overflow: hidden; box-shadow: 0 10px 26px rgba(0, 0, 0, .06); }
.ds-header { display: flex; align-items: center; gap: 8px; padding: 12px 14px; font-size: 14px; font-weight: 600; color: var(--color-text-1); border-bottom: 1px solid rgba(var(--primary-6), 0.1); }
.ds-toggle-all { margin-left: auto; font-size: 12px; color: var(--color-text-4); background: transparent; border: 0; cursor: pointer; font-family: inherit; }
.ds-toggle-all:hover { color: rgb(var(--primary-6)); }
.ds-list { padding: 4px 0; }
.ds-item { display: flex; align-items: center; gap: 10px; width: 100%; padding: 10px 14px; border: 0; background: transparent; cursor: pointer; font-family: inherit; text-align: left; transition: background 0.1s; }
.ds-item:hover { background: rgba(var(--primary-6), 0.04); }
.ds-item.selected { background: rgba(var(--primary-6), 0.08); }
.ds-check { display: flex; align-items: center; justify-content: center; width: 20px; height: 20px; border-radius: 6px; border: 2px solid var(--color-border-2); flex-shrink: 0; color: #fff; }
.ds-item.selected .ds-check { background: rgb(var(--primary-6)); border-color: rgb(var(--primary-6)); }
.ds-platform { font-size: 13px; font-weight: 500; color: var(--color-text-2); }
.ds-name { font-size: 12px; color: var(--color-text-4); }
.ds-action { padding: 10px 14px; border-top: 1px solid rgba(var(--primary-6), 0.1); }
.ds-confirm { padding: 7px 16px; font-size: 12px; font-weight: 650; color: #fff; background: linear-gradient(110deg, rgb(var(--primary-6)), #755bff); border: 0; border-radius: 8px; cursor: pointer; font-family: inherit; }
.ds-confirm:hover { opacity: 0.9; }
.ds-confirm:disabled { opacity: 0.3; cursor: default; }
</style>
