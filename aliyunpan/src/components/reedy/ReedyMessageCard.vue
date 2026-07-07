<script setup lang='ts'>
defineProps<{
  content: string
  role: 'user' | 'assistant'
  isStreaming?: boolean
}>()

defineEmits<{
  (e: 'copy', content: string): void
  (e: 'retry'): void
  (e: 'cfi-click', cfi: string): void
}>()
</script>

<template>
  <div :class="['reedy-message', role]">
    <div class="message-bubble" v-html="content" />
    <div v-if="isStreaming" class="blinking-cursor">|</div>
    <div v-if="role === 'assistant' && !isStreaming" class="message-actions">
      <a-button size="mini" type="text" @click="$emit('copy', content)">
        <template #icon><icon-copy /></template>
      </a-button>
    </div>
  </div>
</template>

<style scoped>
.reedy-message {
  display: flex;
  flex-direction: column;
  margin: 6px 0;
  max-width: 90%;
}
.reedy-message.user {
  align-self: flex-end;
  align-items: flex-end;
}
.reedy-message.assistant {
  align-self: flex-start;
}
.message-bubble {
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 13px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-wrap: break-word;
}
.user .message-bubble {
  background: rgb(var(--primary-6));
  color: #fff;
}
.assistant .message-bubble {
  background: var(--color-bg-3);
  color: var(--color-text-1);
}
.message-actions {
  display: flex;
  gap: 4px;
  margin-top: 2px;
  opacity: 0;
  transition: opacity 0.2s;
}
.reedy-message:hover .message-actions {
  opacity: 1;
}
.blinking-cursor {
  display: inline;
  animation: blink 1s step-end infinite;
  color: rgb(var(--primary-6));
  font-weight: bold;
}
@keyframes blink {
  50% { opacity: 0; }
}
</style>
