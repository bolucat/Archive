<script setup lang='ts'>
import { ref } from 'vue'
import IconFont from '../../components/IconFont.vue'

defineProps<{
  aiMode: 'ask' | 'chat'
  disabled: boolean
  streaming: boolean
}>()

const emit = defineEmits<{
  (e: 'send', text: string): void
  (e: 'stop'): void
  (e: 'toggle-mode'): void
  (e: 'clear'): void
}>()

const inputText = ref('')

function handleSend() {
  const text = inputText.value.trim()
  if (!text) return
  emit('send', text)
  inputText.value = ''
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    handleSend()
  }
}
</script>

<template>
  <div class="reedy-composer">
    <div class="composer-actions">
      <a-button size="mini" :type="aiMode === 'ask' ? 'primary' : 'default'" @click="$emit('toggle-mode')" :disabled="disabled">
        {{ aiMode === 'ask' ? '提问' : '聊天' }}
      </a-button>
      <a-button size="mini" type="text" @click="$emit('clear')" :disabled="disabled" title="清空对话">
        <template #icon><IconFont name="icondelete" /></template>
      </a-button>
    </div>
    <div class="composer-input-row">
      <a-textarea
        v-model:model-value="inputText"
        :auto-size="{ minRows: 1, maxRows: 4 }"
        placeholder="输入问题..."
        :disabled="disabled"
        @keydown="handleKeydown"
      />
      <a-button
        v-if="!streaming"
        type="primary"
        size="small"
        :disabled="disabled || !inputText.trim()"
        @click="handleSend"
      >
        <template #icon><IconFont name="iconsend" /></template>
      </a-button>
      <a-button
        v-else
        status="danger"
        size="small"
        @click="$emit('stop')"
      >停止</a-button>
    </div>
  </div>
</template>

<style scoped>
.reedy-composer {
  border-top: 1px solid var(--color-border-2);
  padding: 8px;
}
.composer-actions {
  display: flex;
  gap: 4px;
  margin-bottom: 6px;
}
.composer-input-row {
  display: flex;
  gap: 6px;
  align-items: flex-end;
}
.composer-input-row :deep(.arco-textarea) {
  flex: 1;
}
.composer-input-row :deep(.arco-textarea textarea) {
  font-size: 13px;
}
</style>
