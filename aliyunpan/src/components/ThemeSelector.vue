<template>
  <span class="theme-btn-wrapper">
    <a-button type="text" shape="circle" title="切换主题" @click="visible = true">
      <svg viewBox="0 0 24 24" width="17" height="17" stroke="currentColor" fill="none" stroke-width="1.8">
        <circle cx="12" cy="12" r="10"/>
        <path d="M12 2a10 10 0 0 1 0 20"/>
        <path d="M12 2a10 10 0 0 0 0 20"/>
      </svg>
    </a-button>
    <a-modal v-model:visible="visible" title="主题选择" :footer="false" :width="380">
      <div class="theme-grid">
        <div
          v-for="t in defaultThemes"
          :key="t.id"
          :class="['theme-item', currentTheme === t.id ? 'active' : '']"
          :title="t.name"
          @click="selectTheme(t.id)"
        >
          <div class="theme-circle" :style="{ background: t.primaryColor }"></div>
          <span class="theme-name">{{ t.name }}</span>
        </div>
      </div>

      <a-divider>自定义</a-divider>

      <div class="custom-theme">
        <div class="custom-row">
          <span class="custom-label">主色调</span>
          <input type="color" v-model="customPrimary" class="custom-color" />
        </div>
        <div class="custom-row">
          <span class="custom-label">暗色模式</span>
          <a-switch v-model="customDark" size="small" />
        </div>
        <a-button type="primary" size="small" long @click="applyCustom">应用自定义主题</a-button>
      </div>
    </a-modal>
  </span>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { defaultThemes, applyTheme, saveCustomTheme, type ThemeDef } from '../module/theme/themes'

const visible = ref(false)
const currentTheme = ref('green')
const customPrimary = ref('#22ad38')
const customDark = ref(true)

function rgbToStr(hex: string): string {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgb(${r}, ${g}, ${b})`
}

function selectTheme(id: string) {
  currentTheme.value = id
  applyTheme(id)
}

function applyCustom() {
  const id = 'custom_' + Date.now()
  saveCustomTheme(id, {
    primaryColor: rgbToStr(customPrimary.value),
    fontColor: null,
    isDark: customDark.value,
  })
  currentTheme.value = id
  applyTheme(id)
}

// init default theme
selectTheme('green')
</script>

<style scoped>
.theme-btn-wrapper {
  display: inline-flex;
  align-items: center;
}

.theme-grid {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 10px;
  margin-bottom: 8px;
}

.theme-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 8px 4px;
  border-radius: 8px;
  cursor: pointer;
  border: 2px solid transparent;
  transition: all 0.15s;
}

.theme-item:hover {
  background: var(--color-fill-2);
}

.theme-item.active {
  border-color: rgb(var(--primary-6));
  background: var(--color-fill-2);
}

.theme-circle {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.2);
}

.theme-name {
  font-size: 11px;
  color: var(--color-text-2);
}

.custom-theme {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.custom-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.custom-label {
  font-size: 13px;
  color: var(--color-text-2);
  width: 60px;
}

.custom-color {
  width: 40px;
  height: 28px;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  padding: 0;
}
</style>
