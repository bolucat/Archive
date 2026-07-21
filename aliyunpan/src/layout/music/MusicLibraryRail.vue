<script setup lang="ts">
import { Disc3, Folder, Heart, House, ListMusic, Mic2, Music, Server } from 'lucide-vue-next'
import useMusicLibraryStore, { type MusicSubTab } from '../../store/musiclibrary'
import LibraryScanPanel from '../../components/LibraryScanPanel.vue'

defineProps<{
  selectedScanUserIds: string[]
  scanAccountOptions: { value: string; label: string }[]
  lastScanText: string
}>()

const emit = defineEmits<{
  (e: 'select-tab', tab: MusicSubTab): void
  (e: 'update:selectedScanUserIds', value: string[]): void
  (e: 'start-scan'): void
  (e: 'stop-scan'): void
  (e: 'clear-library'): void
  (e: 'import-local-songs'): void
}>()

const musicStore = useMusicLibraryStore()
const tabs = [
  { key: 'home' as const, label: '首页', icon: House },
  { key: 'all' as const, label: '歌曲', icon: ListMusic },
  { key: 'artists' as const, label: '艺人', icon: Mic2 },
  { key: 'albums' as const, label: '专辑', icon: Disc3 },
  { key: 'folders' as const, label: '文件夹', icon: Folder },
  { key: 'server' as const, label: '服务器音乐', icon: Server },
  { key: 'fav' as const, label: '收藏', icon: Heart }
]
</script>

<template>
  <aside class="music-rail">
    <div class="music-rail-brand">
      <span class="music-rail-brand-icon"><Music :size="22" :stroke-width="1.5" /></span>
      <span>
        <strong>音乐</strong>
        <small>{{ musicStore.totalCount }} 首网盘音乐</small>
      </span>
    </div>

    <button v-for="tab in tabs" :key="tab.key" :class="['music-rail-tab', musicStore.subTab === tab.key ? 'active' : '']" @click="emit('select-tab', tab.key)">
      <component :is="tab.icon" :size="19" :stroke-width="1.6" />
      <span>{{ tab.label }}</span>
    </button>

    <LibraryScanPanel
      :drive-options="scanAccountOptions"
      :selected-ids="selectedScanUserIds"
      :is-scanning="musicStore.isScanning"
      :scanning-status-text="musicStore.scanLabel || '扫描中...'"
      :idle-status-text="lastScanText"
      import-label="导入本地歌曲"
      clear-confirm-text="确定清空整个音乐资料库？此操作不可恢复"
      :clear-disabled="!musicStore.totalCount"
      @update:selected-ids="(v) => emit('update:selectedScanUserIds', v)"
      @start-scan="emit('start-scan')"
      @stop-scan="emit('stop-scan')"
      @import-local="emit('import-local-songs')"
      @clear-library="emit('clear-library')"
    />
  </aside>
</template>

<style scoped lang="less">
.music-rail {
  flex: 0 0 218px;
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 18px 0 18px 18px;
  padding: 16px 12px 14px;
  color: #fff;
  background:
    radial-gradient(circle at 42% 0%, rgba(0,245,212,.16), transparent 34%),
    radial-gradient(circle at 0% 86%, rgba(244,210,138,.07), transparent 26%),
    linear-gradient(180deg, rgba(14,16,20,.62), rgba(8,9,11,.82));
  border: 1px solid rgba(255,255,255,.085);
  border-radius: 28px;
  overflow: hidden;
  box-shadow: 0 24px 80px rgba(0,0,0,.34), inset 0 1px 0 rgba(255,255,255,.075);
  backdrop-filter: blur(34px) saturate(1.24);
}
.music-rail::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  background:
    linear-gradient(90deg, rgba(255,255,255,.030) 0 1px, transparent 1px 44px),
    linear-gradient(0deg, rgba(255,255,255,.020) 0 1px, transparent 1px 42px),
    linear-gradient(180deg, rgba(255,255,255,.07), transparent 18%),
    radial-gradient(circle at 50% 22%, rgba(255,255,255,.06), transparent 34%);
  opacity: .9;
}
.music-rail > * {
  position: relative;
  z-index: 1;
}
.music-rail-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
  padding: 10px 8px 14px;
  border: 1px solid rgba(255,255,255,.065);
  border-radius: 22px;
  background: linear-gradient(142deg, rgba(255,255,255,.070), rgba(255,255,255,.030));
  box-shadow: inset 0 1px 0 rgba(255,255,255,.075), 0 16px 44px rgba(0,0,0,.16);
}
.music-rail-brand-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 14px;
  color: #081111;
  background: linear-gradient(135deg, #8ff5ea, #f4d28a);
  box-shadow: 0 14px 32px rgba(0,245,212,.12);
}
.music-rail-brand strong,
.music-rail-brand small {
  display: block;
}
.music-rail-brand strong {
  font-size: 18px;
  letter-spacing: 0;
}
.music-rail-brand small {
  margin-top: 3px;
  color: rgba(255,255,255,.44);
  font-size: 12px;
}
.music-rail-tab {
  height: 46px;
  display: flex;
  align-items: center;
  gap: 11px;
  border: 1px solid transparent;
  border-radius: 999px;
  padding: 0 12px;
  color: rgba(255,255,255,.62);
  background: rgba(255,255,255,0);
  font: inherit;
  font-size: 13px;
  font-weight: 760;
  cursor: pointer;
  transition: background .18s, color .18s, box-shadow .18s, transform .18s;
}
.music-rail-tab:hover,
.music-rail-tab.active {
  color: #fff;
  background: rgba(255,255,255,.072);
}
.music-rail-tab.active {
  border-color: rgba(0,245,212,.26);
  background: rgba(0,245,212,.090);
  box-shadow: inset 0 0 0 1px rgba(0,245,212,.12), 0 12px 30px rgba(0,245,212,.075);
}
.music-rail-tab:hover {
  transform: translateY(-1px);
}
.music-rail :deep(.arco-btn) {
  border-color: rgba(255,255,255,.08);
  border-radius: 12px;
  background: rgba(255,255,255,.055);
  color: rgba(255,255,255,.72);
}
.music-rail :deep(.arco-btn-primary) {
  border: 0;
  background: linear-gradient(135deg, #00f5d4, #f4d28a);
  color: #07110f;
  font-weight: 800;
}

</style>

<style>
body:not([arco-theme='dark']) .music-rail {
  color: var(--color-text-1);
  background: var(--color-bg-1);
  border-right: 1px solid var(--color-neutral-3);
  box-shadow: none;
  backdrop-filter: none;
}

body:not([arco-theme='dark']) .music-rail::before {
  display: none;
}

body:not([arco-theme='dark']) .music-rail-brand {
  border-color: var(--color-border-2);
  background: var(--color-bg-1);
  box-shadow: none;
  backdrop-filter: none;
}

body:not([arco-theme='dark']) .music-rail-brand small {
  color: var(--color-text-2);
}

body:not([arco-theme='dark']) .music-rail-tab {
  color: var(--color-text-2);
  background: transparent;
  box-shadow: none;
}

body:not([arco-theme='dark']) .music-rail-tab:hover,
body:not([arco-theme='dark']) .music-rail-tab.active {
  color: var(--color-text-1);
  background: var(--color-fill-2);
  box-shadow: none;
}

body:not([arco-theme='dark']) .music-rail .arco-btn {
  border-color: var(--color-border-2);
  background: var(--color-bg-1);
  color: var(--color-text-1);
}

body:not([arco-theme='dark']) .music-rail .arco-btn-primary {
  color: #fff;
  background: rgb(var(--primary-6));
}
</style>
