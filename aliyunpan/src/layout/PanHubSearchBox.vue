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
      <button v-if="showAiToggle" class="ph-box-btn ph-box-btn-ai" :class="{ active: aiEnabled }" type="button" :title="aiEnabled ? '关闭 AI 搜索' : '开启 AI 搜索'" @click="emit('toggle-ai')"><Sparkles :size="15" :stroke-width="1.8" /></button>
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
</style>
