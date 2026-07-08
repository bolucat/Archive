<script setup lang="ts">
import { ref } from 'vue'
import { Search, X, RotateCcw, Sparkles, Pause } from 'lucide-vue-next'

const props = defineProps<{ modelValue: string; loading: boolean; searched: boolean; placeholder?: string; aiEnabled?: boolean; showAiToggle?: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: string]; search: []; reset: []; pause: []; 'toggle-ai': [] }>()
const isFocused = ref(false)
const inputEl = ref<HTMLInputElement|null>(null)

function handleSearch() { (document.activeElement as HTMLElement)?.blur(); emit('search') }
function handleKeydown(e: KeyboardEvent) { if(e.key==='Enter'){e.preventDefault();e.stopPropagation();handleSearch()} }
function handleReset() { emit('update:modelValue',''); emit('reset') }
</script>

<template>
  <div class="ph-box" :class="{focused:isFocused}">
    <Search :size="20" :stroke-width="1.8" class="ph-box-icon" />
    <input ref="inputEl" :value="modelValue" type="text" class="ph-box-input" :placeholder="placeholder||'搜索全网网盘资源...'" autocomplete="off" spellcheck="false"
      @input="emit('update:modelValue',($event.target as HTMLInputElement).value)" @keydown="handleKeydown" @focus="isFocused=true" @blur="isFocused=false" />
    <div class="ph-box-actions">
      <button v-if="modelValue&&!loading&&!searched" class="ph-box-btn" type="button" title="清除" @click="emit('update:modelValue','');inputEl?.focus()"><X :size="16" :stroke-width="2" /></button>
      <button v-if="loading" class="ph-box-btn ph-box-btn-stop" type="button" title="停止搜索" @click="emit('pause')"><Pause :size="15" :stroke-width="2.5" /></button>
      <button v-if="searched&&!loading" class="ph-box-btn ph-box-btn-reset" type="button" title="重置" @click="handleReset"><RotateCcw :size="16" :stroke-width="1.5" /></button>
      <button v-if="showAiToggle" class="ph-box-btn ph-box-btn-ai" :class="{ active: aiEnabled }" type="button" :title="aiEnabled ? '关闭 AI 搜索（Pro）' : '开启 AI 搜索（Pro）'" @click="emit('toggle-ai')">
        <Sparkles :size="15" :stroke-width="1.8" />
        <span class="ph-pro-corner">Pro</span>
      </button>
      <button class="ph-box-btn ph-box-btn-search" type="button" title="搜索" :disabled="!modelValue.trim()" @click="handleSearch"><Search :size="16" :stroke-width="2" /></button>
    </div>
  </div>
</template>

<style scoped>
.ph-box{display:flex;align-items:center;gap:12px;width:100%;max-width:720px;margin:0 auto;padding:14px 20px;background:var(--color-bg-2);border:1px solid var(--color-border-2);border-radius:12px;transition:border-color .2s,box-shadow .2s}
.ph-box.focused{border-color:rgb(var(--primary-6));box-shadow:0 0 0 3px rgba(var(--primary-6),.12)}
.ph-box-icon{flex-shrink:0;color:var(--color-text-4)}
.ph-box-input{flex:1;min-width:0;padding:0;color:var(--color-text-1);font-size:18px;font-weight:400;line-height:28px;background:transparent;border:0;outline:none}
.ph-box-input::placeholder{color:var(--color-text-4)}
.ph-box-actions{display:flex;align-items:center;gap:6px;flex-shrink:0}
.ph-box-btn{display:inline-flex;align-items:center;justify-content:center;width:34px;height:34px;padding:0;color:var(--color-text-3);background:var(--color-fill-1);border:0;border-radius:8px;cursor:pointer;transition:background .15s,color .15s}
.ph-box-btn{position:relative}
.ph-box-btn:hover:not(:disabled){background:var(--color-fill-2);color:var(--color-text-2)}
.ph-box-btn:disabled{opacity:.4;cursor:default}
.ph-box-btn-search{color:#fff;background:rgb(var(--primary-6))}
.ph-box-btn-search:hover:not(:disabled){background:rgb(var(--primary-5));color:#fff}
.ph-box-btn-reset{color:#f56c6c;background:rgba(245,108,108,.1)}
.ph-box-btn-reset:hover{background:rgba(245,108,108,.2)}
.ph-box-btn-stop{color:#e6a23c;background:rgba(230,162,60,.12)}
.ph-box-btn-stop:hover{background:rgba(230,162,60,.22)}
.ph-box-btn-ai{color:var(--color-text-3)}
.ph-box-btn-ai:hover,.ph-box-btn-ai.active{color:rgb(var(--primary-6));background:rgba(var(--primary-6),.1)}
.ph-pro-corner{position:absolute;top:-7px;right:-8px;display:inline-flex;align-items:center;justify-content:center;height:14px;padding:0 5px;border-radius:999px;background:linear-gradient(135deg,#f59e0b,#f97316);color:#fff;font-size:8px;font-weight:700;line-height:1;box-shadow:0 2px 6px rgba(245,158,11,.28)}
</style>

<style>
body[arco-theme='dark'] .ph-box {
  background: rgba(255,255,255,.055);
  border: 1px solid rgba(255,255,255,.075);
  border-radius: 16px;
  backdrop-filter: blur(18px) saturate(1.2);
}

body[arco-theme='dark'] .ph-box.focused {
  border-color: rgba(0,245,212,.22);
  box-shadow: 0 0 0 3px rgba(0,245,212,.08);
}

body[arco-theme='dark'] .ph-box-icon {
  color: rgba(255,255,255,.42);
}

body[arco-theme='dark'] .ph-box-input {
  color: #fff;
}

body[arco-theme='dark'] .ph-box-input::placeholder {
  color: rgba(255,255,255,.36);
}

body[arco-theme='dark'] .ph-box-btn {
  color: rgba(255,255,255,.60);
  background: rgba(255,255,255,.055);
}

body[arco-theme='dark'] .ph-box-btn:hover:not(:disabled) {
  background: rgba(255,255,255,.085);
  color: #fff;
}

body[arco-theme='dark'] .ph-box-btn-search {
  color: #07110f;
  background: linear-gradient(135deg, var(--app-mineradio-accent, #00f5d4), var(--app-mineradio-champagne, #f4d28a));
  font-weight: 800;
}

body[arco-theme='dark'] .ph-box-btn-search:hover:not(:disabled) {
  background: linear-gradient(135deg, #00f5d4, #f4d28a);
  color: #07110f;
}

body[arco-theme='dark'] .ph-box-btn-reset {
  color: #ff8f9d;
  background: rgba(255,143,157,.10);
}

body[arco-theme='dark'] .ph-box-btn-reset:hover {
  background: rgba(255,143,157,.18);
}

body[arco-theme='dark'] .ph-box-btn-stop {
  color: #f4d28a;
  background: rgba(244,210,138,.10);
}

body[arco-theme='dark'] .ph-box-btn-stop:hover {
  background: rgba(244,210,138,.18);
}

body[arco-theme='dark'] .ph-box-btn-ai {
  color: rgba(255,255,255,.60);
}

body[arco-theme='dark'] .ph-box-btn-ai:hover,
body[arco-theme='dark'] .ph-box-btn-ai.active {
  color: var(--app-mineradio-accent, #00f5d4);
  background: rgba(0,245,212,.10);
}
</style>
