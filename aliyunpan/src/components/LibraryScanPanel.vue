<script setup lang="ts">
import { computed } from 'vue'
import { Upload } from 'lucide-vue-next'
import IconFont from './IconFont.vue'
import { t } from '../i18n'

interface DriveOption {
  value: string
  label: string
}

const props = withDefaults(
  defineProps<{
    driveOptions: DriveOption[]
    selectedIds: string[]
    isScanning?: boolean
    title?: string
    startLabel?: string
    stopLabel?: string
    scanningStatusText?: string
    idleStatusText?: string
    importLabel?: string
    importIconName?: string
    importDisabled?: boolean
    clearLabel?: string
    clearConfirmText?: string
    clearDisabled?: boolean
  }>(),
  {
    isScanning: false,
    title: '',
    startLabel: '',
    stopLabel: '',
    scanningStatusText: '',
    idleStatusText: '',
    importLabel: '',
    importIconName: '',
    importDisabled: false,
    clearLabel: '',
    clearConfirmText: '',
    clearDisabled: false
  }
)

const emit = defineEmits<{
  (e: 'update:selectedIds', value: string[]): void
  (e: 'start-scan'): void
  (e: 'stop-scan'): void
  (e: 'import-local'): void
  (e: 'clear-library'): void
}>()

function toggleId(id: string) {
  if (props.isScanning) return
  const set = new Set(props.selectedIds)
  if (set.has(id)) set.delete(id)
  else set.add(id)
  emit('update:selectedIds', Array.from(set))
}

function selectAll() {
  if (props.isScanning) return
  emit('update:selectedIds', props.driveOptions.map((o) => o.value))
}

function clearAll() {
  if (props.isScanning) return
  emit('update:selectedIds', [])
}

const titleText = computed(() => props.title || t('scan.cloudDriveScan'))
const startText = computed(() => props.startLabel || t('scan.start'))
const stopText = computed(() => props.stopLabel || t('scan.stop'))
const clearText = computed(() => props.clearLabel || t('scan.clearLibrary'))
const driveButtonText = computed(() => props.selectedIds.length === props.driveOptions.length ? t('scan.allDrives') : `${props.selectedIds.length} ${t('scan.drives')}`)
</script>

<template>
  <div class="library-scan-panel">
    <div class="library-scan-label" :title="titleText">{{ titleText }}</div>

    <a-dropdown trigger="click" :disabled="isScanning">
      <a-button size="mini" long class="library-scan-drive-btn" :title="driveButtonText">
        {{ driveButtonText }}
      </a-button>
      <template #content>
        <div class="library-scan-drive-list" :aria-disabled="isScanning">
          <label
            v-for="opt in driveOptions"
            :key="opt.value"
            :class="['library-scan-drive-card', selectedIds.includes(opt.value) ? 'selected' : '', isScanning ? 'disabled' : '']"
            @click.stop
          >
            <a-checkbox :disabled="isScanning" :model-value="selectedIds.includes(opt.value)" @change="toggleId(opt.value)" />
            <span :title="opt.label">{{ opt.label }}</span>
          </label>
          <div v-if="!driveOptions.length" class="library-scan-drive-empty">{{ t('scan.noDrives') }}</div>
          <div class="library-scan-drive-footer">
            <a-link size="small" :disabled="isScanning" @click="selectAll">{{ t('scan.selectAll') }}</a-link>
            <a-link size="small" :disabled="isScanning" @click="clearAll">{{ t('scan.clear') }}</a-link>
          </div>
        </div>
      </template>
    </a-dropdown>

    <a-button v-if="isScanning" status="warning" size="mini" long :title="stopText" @click="emit('stop-scan')">{{ stopText }}</a-button>
    <a-button v-else type="primary" size="mini" long :title="startText" @click="emit('start-scan')">{{ startText }}</a-button>

    <button v-if="importLabel" class="library-scan-import-btn" type="button" :title="importLabel" :disabled="importDisabled" @click="emit('import-local')">
      <IconFont v-if="importIconName" :name="importIconName" />
      <Upload v-else :size="14" :stroke-width="1.8" />
      <span>{{ importLabel }}</span>
    </button>

    <a-popconfirm v-if="clearConfirmText" :content="clearConfirmText" @ok="emit('clear-library')">
      <a-button size="mini" long status="danger" :title="clearText" :disabled="clearDisabled">{{ clearText }}</a-button>
    </a-popconfirm>
    <a-button v-else size="mini" long status="danger" :title="clearText" :disabled="clearDisabled" @click="emit('clear-library')">{{ clearText }}</a-button>

    <slot name="extra-actions" />

  </div>
</template>

<style scoped lang="less">
.library-scan-panel {
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding: 14px;
  border-radius: 22px;
  border: 1px solid rgba(255, 255, 255, .078);
  background:
    radial-gradient(circle at 10% 0%, rgba(255, 255, 255, .09), transparent 32%),
    linear-gradient(145deg, rgba(255, 255, 255, .052), rgba(255, 255, 255, .026));
  box-shadow: 0 18px 48px rgba(0, 0, 0, .18), inset 0 1px 0 rgba(255, 255, 255, .065);
  backdrop-filter: blur(22px) saturate(1.14);
  box-sizing: border-box;
}

.library-scan-label {
  overflow: hidden;
  color: rgba(0, 245, 212, .58);
  font-size: 10.5px;
  font-weight: 800;
  letter-spacing: .12em;
  text-overflow: ellipsis;
  text-transform: uppercase;
  white-space: nowrap;
}

.library-scan-drive-list {
  display: flex;
  flex-direction: column;
  gap: 7px;
  min-width: 238px;
  max-height: 178px;
  overflow: auto;
  padding: 10px 8px 10px 10px;
  border: 1px solid rgba(255, 255, 255, .08);
  border-radius: 16px;
  background:
    radial-gradient(circle at 12% 0%, rgba(0, 245, 212, .12), transparent 36%),
    linear-gradient(145deg, rgba(11, 13, 17, .86), rgba(6, 7, 11, .92));
  box-shadow: 0 22px 70px rgba(0, 0, 0, .34), inset 0 1px 0 rgba(255, 255, 255, .08);
  backdrop-filter: blur(24px) saturate(1.18);
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 245, 212, .26) transparent;
}
.library-scan-drive-list::-webkit-scrollbar {
  width: 3px;
}
.library-scan-drive-list::-webkit-scrollbar-thumb {
  border-radius: 3px;
  background: rgba(0, 245, 212, .26);
}

.library-scan-drive-card {
  min-height: 36px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 9px;
  border: 1px solid rgba(255, 255, 255, .075);
  border-radius: 12px;
  color: rgba(255, 255, 255, .70);
  background: rgba(255, 255, 255, .040);
  cursor: pointer;
  transition: background .18s, border-color .18s, color .18s, transform .18s;
}
.library-scan-drive-card:hover,
.library-scan-drive-card.selected {
  color: #fff;
  border-color: rgba(0, 245, 212, .24);
  background: rgba(0, 245, 212, .075);
}
.library-scan-drive-card:hover {
  transform: translateY(-1px);
}
.library-scan-drive-card.disabled {
  opacity: .58;
  cursor: default;
  transform: none;
}
.library-scan-drive-card span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11.5px;
  font-weight: 720;
}

.library-scan-drive-empty {
  padding: 10px;
  border: 1px dashed rgba(255, 255, 255, .10);
  border-radius: 12px;
  color: rgba(255, 255, 255, .36);
  font-size: 11px;
  text-align: center;
}

.library-scan-drive-footer {
  display: flex;
  justify-content: space-between;
  padding: 2px 3px 0;
}

.library-scan-footer {
  min-height: 36px;
  display: flex;
  align-items: center;
  padding: 0 8px;
  color: rgba(255, 255, 255, .36);
  font-size: 11px;
  line-height: 1.4;
}

.library-scan-panel :deep(.arco-btn) {
  border-color: rgba(255, 255, 255, .08);
  border-radius: 12px;
  background: rgba(255, 255, 255, .055);
  color: rgba(255, 255, 255, .72);
}
.library-scan-panel :deep(.arco-btn-primary) {
  border: 0;
  background: linear-gradient(135deg, #00f5d4, #f4d28a);
  color: #07110f;
  font-weight: 800;
}
</style>

<style>
.library-scan-import-btn {
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  border: 1px solid rgba(244, 210, 138, .18);
  border-radius: 12px;
  color: rgba(255, 247, 222, .82);
  background:
    radial-gradient(circle at 20% 0%, rgba(244, 210, 138, .18), transparent 42%),
    rgba(255, 255, 255, .045);
  font: inherit;
  font-size: 12px;
  font-weight: 790;
  cursor: pointer;
  transition: transform .18s, border-color .18s, background .18s, color .18s;
}
.library-scan-import-btn span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.library-scan-import-btn:hover {
  color: #fff8df;
  border-color: rgba(244, 210, 138, .34);
  background: rgba(244, 210, 138, .10);
  transform: translateY(-1px);
}
.library-scan-import-btn:disabled {
  opacity: .58;
  cursor: default;
  transform: none;
}

body:not([arco-theme='dark']) .library-scan-panel {
  border-color: var(--color-border-2);
  background: var(--color-bg-1);
  box-shadow: none;
  backdrop-filter: none;
}
body:not([arco-theme='dark']) .library-scan-label,
body:not([arco-theme='dark']) .library-scan-footer {
  color: var(--color-text-2);
}
body:not([arco-theme='dark']) .library-scan-panel .arco-btn {
  border-color: var(--color-border-2);
  background: var(--color-bg-1);
  color: var(--color-text-1);
}
body:not([arco-theme='dark']) .library-scan-panel .arco-btn-primary {
  color: #fff;
  background: rgb(var(--primary-6));
}
body:not([arco-theme='dark']) .library-scan-import-btn {
  border-color: var(--color-border-2);
  background: var(--color-bg-1);
  color: var(--color-text-1);
}
body:not([arco-theme='dark']) .library-scan-import-btn:hover {
  border-color: rgb(var(--primary-6));
  color: rgb(var(--primary-6));
  background: var(--color-fill-2);
}
body:not([arco-theme='dark']) .library-scan-drive-list {
  border-color: var(--color-border-2);
  background: var(--color-bg-1);
  box-shadow: 0 8px 22px rgba(0, 0, 0, .08);
  backdrop-filter: none;
}
body:not([arco-theme='dark']) .library-scan-drive-card {
  border-color: var(--color-border-2);
  background: var(--color-bg-1);
  color: var(--color-text-2);
}
body:not([arco-theme='dark']) .library-scan-drive-card:hover,
body:not([arco-theme='dark']) .library-scan-drive-card.selected {
  color: var(--color-text-1);
  border-color: rgb(var(--primary-6));
  background: var(--color-fill-2);
}

#xbybody .library-scan-panel {
  border: 1px solid rgba(255, 255, 255, .078) !important;
  background:
    radial-gradient(circle at 10% 0%, rgba(255, 255, 255, .09), transparent 32%),
    linear-gradient(145deg, rgba(255, 255, 255, .052), rgba(255, 255, 255, .026)) !important;
  box-shadow: 0 18px 48px rgba(0, 0, 0, .18), inset 0 1px 0 rgba(255, 255, 255, .065) !important;
  backdrop-filter: blur(22px) saturate(1.14) !important;
}
body:not([arco-theme='dark']) #xbybody .library-scan-panel {
  border: 1px solid var(--color-border-2) !important;
  background: var(--color-bg-1) !important;
  box-shadow: none !important;
  backdrop-filter: none !important;
}

#xbybody .music-rail .library-scan-panel,
#xbybody .book-sidebar .library-scan-panel,
#xbybody .media-library-nav .library-scan-panel {
  border: 1px solid rgba(255, 255, 255, .078) !important;
  background:
    radial-gradient(circle at 10% 0%, rgba(255, 255, 255, .09), transparent 32%),
    linear-gradient(145deg, rgba(255, 255, 255, .052), rgba(255, 255, 255, .026)) !important;
  box-shadow: 0 18px 48px rgba(0, 0, 0, .18), inset 0 1px 0 rgba(255, 255, 255, .065) !important;
  backdrop-filter: blur(22px) saturate(1.14) !important;
}

#xbybody .music-rail .library-scan-panel .arco-btn,
#xbybody .book-sidebar .library-scan-panel .arco-btn,
#xbybody .media-library-nav .library-scan-panel .arco-btn {
  width: 100% !important;
  height: 32px !important;
  min-height: 32px !important;
  justify-content: center !important;
  border: 1px solid rgba(255, 255, 255, .08) !important;
  border-radius: 999px !important;
  background: rgba(255, 255, 255, .055) !important;
  color: rgba(255, 255, 255, .72) !important;
  font-size: 12px !important;
  font-weight: 500 !important;
  line-height: 1 !important;
  box-shadow: none !important;
  transform: none !important;
}

#xbybody .music-rail .library-scan-panel .arco-btn-primary,
#xbybody .book-sidebar .library-scan-panel .arco-btn-primary,
#xbybody .media-library-nav .library-scan-panel .arco-btn-primary {
  border-color: transparent !important;
  background: linear-gradient(135deg, #00f5d4, #f4d28a) !important;
  color: #07110f !important;
  font-weight: 800 !important;
}

#xbybody .music-rail .library-scan-import-btn,
#xbybody .book-sidebar .library-scan-import-btn,
#xbybody .media-library-nav .library-scan-import-btn {
  width: 100% !important;
  height: 32px !important;
  min-height: 32px !important;
  justify-content: center !important;
  border: 1px solid rgba(255, 255, 255, .08) !important;
  border-radius: 999px !important;
  background: rgba(255, 255, 255, .055) !important;
  color: rgba(255, 255, 255, .72) !important;
  font-size: 12px !important;
  font-weight: 500 !important;
  line-height: 1 !important;
  box-shadow: none !important;
  transform: none !important;
}

#xbybody .music-rail .library-scan-import-btn:hover,
#xbybody .book-sidebar .library-scan-import-btn:hover,
#xbybody .media-library-nav .library-scan-import-btn:hover {
  border-color: rgba(0, 245, 212, .24) !important;
  background: rgba(0, 245, 212, .075) !important;
  color: #fff !important;
}

body:not([arco-theme='dark']) #xbybody .music-rail .library-scan-panel .arco-btn,
body:not([arco-theme='dark']) #xbybody .book-sidebar .library-scan-panel .arco-btn,
body:not([arco-theme='dark']) #xbybody .media-library-nav .library-scan-panel .arco-btn {
  border-color: var(--color-border-2) !important;
  background: var(--color-bg-1) !important;
  color: var(--color-text-1) !important;
}

body:not([arco-theme='dark']) #xbybody .music-rail .library-scan-panel .arco-btn-primary,
body:not([arco-theme='dark']) #xbybody .book-sidebar .library-scan-panel .arco-btn-primary,
body:not([arco-theme='dark']) #xbybody .media-library-nav .library-scan-panel .arco-btn-primary {
  border-color: transparent !important;
  background: rgb(var(--primary-6)) !important;
  color: #fff !important;
}

body:not([arco-theme='dark']) #xbybody .music-rail .library-scan-import-btn,
body:not([arco-theme='dark']) #xbybody .book-sidebar .library-scan-import-btn,
body:not([arco-theme='dark']) #xbybody .media-library-nav .library-scan-import-btn {
  border-color: var(--color-border-2) !important;
  background: var(--color-bg-1) !important;
  color: var(--color-text-1) !important;
}

body:not([arco-theme='dark']) #xbybody .music-rail .library-scan-import-btn:hover,
body:not([arco-theme='dark']) #xbybody .book-sidebar .library-scan-import-btn:hover,
body:not([arco-theme='dark']) #xbybody .media-library-nav .library-scan-import-btn:hover {
  border-color: rgb(var(--primary-6)) !important;
  background: var(--color-fill-2) !important;
  color: rgb(var(--primary-6)) !important;
}

#xbybody .music-rail .library-scan-label {
  color: #fff;
  font-size: 13px;
}

#xbybody .music-rail .library-scan-panel .arco-btn:not(.arco-btn-primary),
#xbybody .music-rail .library-scan-import-btn,
#xbybody .music-rail .library-scan-drive-card {
  color: #fff !important;
  font-size: 13px !important;
}

body:not([arco-theme='dark']) #xbybody .music-rail .library-scan-label {
  color: #111827;
}

body:not([arco-theme='dark']) #xbybody .music-rail .library-scan-panel .arco-btn:not(.arco-btn-primary),
body:not([arco-theme='dark']) #xbybody .music-rail .library-scan-import-btn,
body:not([arco-theme='dark']) #xbybody .music-rail .library-scan-drive-card {
  color: #111827 !important;
}
</style>
