<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  chapterTitle: string
  bookName: string
  percentage: number
  readingTime: string
  layoutMode: string
  textColor: string
  showPageBorder: boolean
  currentPage: number
  totalPage: number
  hideFooter: boolean
}>()

const isDouble = computed(() => props.layoutMode === 'double')
const isSingle = computed(() => props.layoutMode !== 'double')
</script>

<template>
  <!-- koodo page-border decorations -->
  <template v-if="showPageBorder">
    <div class="pw-page-border" :style="{ borderColor: textColor }" />
    <div class="pw-page-border-inner" :style="{ borderColor: textColor }" />
    <div class="pw-page-border-header-line" :style="{ backgroundColor: textColor }" />
    <div v-if="!hideFooter" class="pw-page-border-footer-line" :style="{ backgroundColor: textColor }" />
    <div v-if="isDouble" class="pw-page-border-center-line" :style="{ backgroundColor: textColor }" />
  </template>

  <!-- koodo header-container -->
  <div class="pw-header">
    <span class="pw-header-left">{{ chapterTitle || '' }}</span>
    <span class="pw-header-right">{{ bookName }}</span>
  </div>

  <!-- koodo footer-container -->
  <div v-if="!hideFooter" class="pw-footer">
    <template v-if="isDouble">
      <span class="pw-footer-page">{{ currentPage ? `Page ${currentPage * 2 - 1}` : 'Page —' }}</span>
      <span class="pw-footer-page">{{ currentPage ? `Page ${currentPage * 2}` : 'Page —' }}</span>
    </template>
    <template v-else>
      <span>{{ currentPage ? `Page ${currentPage}` : 'Page —' }}<span v-if="totalPage"> / {{ totalPage }}</span></span>
    </template>
  </div>

  <!-- koodo footer-time -->
  <div v-if="readingTime" class="pw-footer-time">
    <span>{{ readingTime }}</span>
    <span style="margin-left: 12px">{{ percentage }}%</span>
  </div>
</template>

<style scoped>
/* koodo page-border decorations */
.pw-page-border {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  border: 4px solid #e74c3c;
  pointer-events: none;
  box-sizing: border-box;
  z-index: 1;
  margin: 5px;
}
.pw-page-border-inner {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  border: 1px solid #e74c3c;
  pointer-events: none;
  box-sizing: border-box;
  z-index: 1;
  margin: 15px;
}
.pw-page-border-header-line {
  position: absolute;
  top: 37px;
  left: 0;
  right: 0;
  height: 1px;
  background-color: #e74c3c;
  pointer-events: none;
  z-index: 1;
  margin-left: 15px;
  margin-right: 15px;
}
.pw-page-border-footer-line {
  position: absolute;
  bottom: 37px;
  left: 0;
  right: 0;
  height: 1px;
  background-color: #e74c3c;
  pointer-events: none;
  z-index: 1;
  margin-left: 15px;
  margin-right: 15px;
}
.pw-page-border-center-line {
  position: absolute;
  top: 37px;
  bottom: 37px;
  left: 50%;
  width: 1px;
  background-color: #e74c3c;
  pointer-events: none;
  z-index: 1;
}

/* koodo header-container */
.pw-header {
  position: absolute;
  top: 13px;
  left: 0;
  right: 0;
  height: 25px;
  display: flex;
  align-items: center;
  font-size: 13px;
  line-height: 30px;
  opacity: 0.3;
  pointer-events: none;
  z-index: 6;
  white-space: nowrap;
}
.pw-header-left,
.pw-header-right {
  width: 50%;
  padding-left: 10%;
  padding-right: 10%;
  text-overflow: ellipsis;
  white-space: nowrap;
  overflow: hidden;
  text-align: center;
  line-height: 30px;
}

/* koodo footer-container */
.pw-footer {
  position: absolute;
  bottom: 14px;
  left: 0;
  right: 0;
  height: 25px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  line-height: 30px;
  opacity: 0.3;
  pointer-events: none;
  z-index: 6;
  white-space: nowrap;
}
.pw-footer-page {
  width: 50%;
  text-overflow: ellipsis;
  white-space: nowrap;
  overflow: hidden;
  text-align: center;
  line-height: 30px;
}

/* koodo footer-time */
.pw-footer-time {
  position: absolute;
  top: 6px;
  left: 20px;
  font-size: 13px;
  letter-spacing: 0.5px;
  opacity: 0.5;
  pointer-events: none;
  z-index: 6;
}
</style>
