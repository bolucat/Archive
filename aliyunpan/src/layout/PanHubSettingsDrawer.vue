<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import { X } from 'lucide-vue-next'

export interface PanHubSettings {
  enabledPlugins: string[]
  enabledChannels: string[]
  concurrency: number
  pluginTimeoutMs: number
}

const props = defineProps<{
  modelValue: PanHubSettings
  open: boolean
  allPlugins: string[]
  allChannels: string[]
}>()

const emit = defineEmits<{ 'update:modelValue': [v: PanHubSettings]; 'update:open': [v: boolean]; save: [] }>()

const inner = ref<PanHubSettings>({ enabledPlugins: [], enabledChannels: [], concurrency: 4, pluginTimeoutMs: 5000 })
const drawerMainRef = ref<HTMLElement | null>(null)
const activeSection = ref<'plugins' | 'channels' | 'performance'>('plugins')

watch(() => props.modelValue, (v) => { if (v) inner.value = JSON.parse(JSON.stringify(v)) }, { immediate: true })

function saveTemp() { emit('update:modelValue', inner.value); emit('save') }
function onSelectAll() { inner.value.enabledPlugins = [...props.allPlugins]; saveTemp() }
function onClearAll() { inner.value.enabledPlugins = []; saveTemp() }
function onSelectAllCh() { inner.value.enabledChannels = [...props.allChannels]; saveTemp() }
function onClearAllCh() { inner.value.enabledChannels = []; saveTemp() }

function onNavClick(section: 'plugins' | 'channels' | 'performance') {
  activeSection.value = section
  const ids = { plugins: 'ps-plugins', channels: 'ps-channels', performance: 'ps-performance' }
  drawerMainRef.value?.querySelector<HTMLElement>(`#${ids[section]}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

function close() { emit('update:open', false); saveTemp() }
</script>

<template>
  <div v-if="open" class="ps-mask" @click.self="close">
    <div class="ps-drawer">
      <header class="ps-header">
        <div><strong>搜索设置</strong><p class="ps-header-sub">修改后自动保存</p></div>
        <button class="ps-close" type="button" @click="close"><X :size="18" :stroke-width="2" /></button>
      </header>

      <div class="ps-body">
        <aside class="ps-nav">
          <button class="ps-nav-link" :class="{ active: activeSection === 'plugins' }" @click="onNavClick('plugins')">插件来源</button>
          <button class="ps-nav-link" :class="{ active: activeSection === 'channels' }" @click="onNavClick('channels')">频道来源</button>
          <button class="ps-nav-link" :class="{ active: activeSection === 'performance' }" @click="onNavClick('performance')">性能并发</button>
        </aside>

        <div ref="drawerMainRef" class="ps-main">
          <section id="ps-plugins" class="ps-section">
            <div class="ps-section-title"><strong>插件来源</strong>
              <div class="ps-section-tools"><button class="ps-btn-sm" @click="onSelectAll">全选</button><button class="ps-btn-sm" @click="onClearAll">全不选</button></div>
            </div>
            <div class="ps-grid">
              <label v-for="name in allPlugins" :key="name" class="ps-item">
                <input type="checkbox" :value="name" v-model="inner.enabledPlugins" @change="saveTemp" /><span>{{ name }}</span>
              </label>
            </div>
          </section>

          <section id="ps-channels" class="ps-section">
            <div class="ps-section-title"><strong>频道来源</strong>
              <div class="ps-section-tools"><button class="ps-btn-sm" @click="onSelectAllCh">全选</button><button class="ps-btn-sm" @click="onClearAllCh">全不选</button></div>
            </div>
            <div class="ps-grid">
              <label v-for="name in allChannels" :key="name" class="ps-item">
                <input type="checkbox" :value="name" v-model="inner.enabledChannels" @change="saveTemp" /><span>{{ name }}</span>
              </label>
            </div>
          </section>

          <section id="ps-performance" class="ps-section">
            <div class="ps-section-title"><strong>性能与并发</strong></div>
            <div class="ps-field">
              <label class="ps-label">插件并发数</label>
              <input type="number" min="1" max="16" v-model.number="inner.concurrency" @change="saveTemp" class="ps-input" />
              <span class="ps-hint">默认 4，范围 1-16</span>
            </div>
            <div class="ps-field">
              <label class="ps-label">插件超时(ms)</label>
              <input type="number" min="1000" step="500" v-model.number="inner.pluginTimeoutMs" @change="saveTemp" class="ps-input" />
              <span class="ps-hint">默认 5000 ms</span>
            </div>
          </section>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ps-mask{position:fixed;inset:0;background:rgba(0,0,0,.4);backdrop-filter:blur(2px);display:flex;justify-content:flex-end;z-index:1000}
.ps-drawer{width:min(460px,92vw);height:100vh;background:var(--color-bg-2);box-shadow:-8px 0 32px rgba(0,0,0,.15);padding:16px;display:flex;flex-direction:column;overflow:hidden;border-left:1px solid var(--color-border)}
.ps-header{display:flex;align-items:center;justify-content:space-between;margin-bottom:10px;padding-bottom:12px;border-bottom:1px solid var(--color-border)}
.ps-header strong{font-size:17px;font-weight:700;color:var(--color-text-1)}
.ps-header-sub{margin:4px 0 0;font-size:12px;color:var(--color-text-4)}
.ps-close{display:flex;align-items:center;justify-content:center;width:32px;height:32px;padding:0;color:var(--color-text-3);background:transparent;border:0;border-radius:8px;cursor:pointer}
.ps-close:hover{background:var(--color-fill-2);color:var(--color-text-1)}
.ps-body{display:grid;grid-template-columns:88px 1fr;gap:10px;min-height:0;flex:1}
.ps-nav{display:flex;flex-direction:column;gap:6px}
.ps-nav-link{display:block;padding:8px 6px;border-radius:9px;border:1px solid var(--color-border);background:var(--color-fill-1);color:var(--color-text-3);font-size:12px;text-align:center;font-weight:600;cursor:pointer;font-family:inherit}
.ps-nav-link:hover{border-color:var(--color-border-2);color:var(--color-text-2)}
.ps-nav-link.active{border-color:rgb(var(--primary-6));background:rgba(var(--primary-6),.1);color:rgb(var(--primary-6))}
.ps-main{min-width:0;height:100%;overflow-y:auto;padding-right:2px}
.ps-section{background:var(--color-fill-1);border:1px solid var(--color-border);border-radius:12px;padding:10px;margin-bottom:10px}
.ps-section-title{display:flex;align-items:center;justify-content:space-between;margin-bottom:8px}
.ps-section-title strong{font-size:14px;font-weight:700;color:var(--color-text-1)}
.ps-section-tools{display:flex;gap:6px}
.ps-btn-sm{padding:4px 8px;font-size:11px;font-weight:600;color:var(--color-text-3);background:var(--color-fill-2);border:1px solid var(--color-border);border-radius:6px;cursor:pointer;font-family:inherit}
.ps-btn-sm:hover{background:var(--color-fill-3);color:var(--color-text-1)}
.ps-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:7px}
.ps-item{display:flex;align-items:center;gap:8px;padding:6px 8px;background:var(--color-fill-2);border:1px solid var(--color-border);border-radius:99px;min-width:0;cursor:pointer}
.ps-item:hover{background:var(--color-fill-3);border-color:var(--color-border-2)}
.ps-item input[type="checkbox"]{width:16px;height:16px;min-width:16px;margin:0;cursor:pointer;accent-color:rgb(var(--primary-6))}
.ps-item span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:12px;color:var(--color-text-2)}
.ps-field{display:flex;flex-direction:column;gap:6px;margin-bottom:10px}
.ps-label{font-size:12px;color:var(--color-text-3);font-weight:600}
.ps-input{width:100%;padding:8px 10px;border:1px solid var(--color-border);background:var(--color-fill-1);border-radius:10px;font-size:13px;color:var(--color-text-1);outline:none}
.ps-input:focus{border-color:rgb(var(--primary-6));box-shadow:0 0 0 3px rgba(var(--primary-6),.12)}
.ps-hint{font-size:11px;color:var(--color-text-4)}
</style>
