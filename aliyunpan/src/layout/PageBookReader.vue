<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { KeyboardState, useAppStore, useKeyboardStore } from '../store'
import { TestAlt, TestKey } from '../utils/keyboardhelper'
import BookReaderModal from './BookReaderModal.vue'
import type { IBookItem } from '../types/book'

const keyboardStore = useKeyboardStore()
keyboardStore.$subscribe((_m: any, state: KeyboardState) => {
  if (TestAlt('f4', state.KeyDownEvent, handleClose)) return
  if (TestAlt('m', state.KeyDownEvent, handleMin)) return
  if (TestAlt('enter', state.KeyDownEvent, handleMax)) return
  if (TestKey('f11', state.KeyDownEvent, handleMax)) return
})

const appStore = useAppStore()

const bookData = computed<IBookItem | null>(() => {
  const raw = appStore.pageEpub
  if (!raw) return null
  return raw as unknown as IBookItem
})

const handleClose = () => {
  if (window.WebToWindow) window.WebToWindow({ cmd: 'close' })
}
const handleMin = () => {
  if (window.WebToWindow) window.WebToWindow({ cmd: 'minsize' })
}
const handleMax = () => {
  if (window.WebToWindow) window.WebToWindow({ cmd: 'maxsize' })
}

onMounted(() => {
  document.title = bookData.value?.title || bookData.value?.file_name || 'BoxPlayer 阅读器'
})

function onReaderClose() {
  if (window.WebToWindow) window.WebToWindow({ cmd: 'close' })
}
</script>

<template>
  <div class="reader-window">
    <BookReaderModal
      :visible="true"
      :book="bookData"
      :source-url-override="(bookData as any)?.sourceUrlOverride"
      @update:visible="onReaderClose"
    />
  </div>
</template>

<style scoped>
.reader-window {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  background: #000;
}
</style>
