<script setup lang='ts'>
import { ref, markRaw } from 'vue'
import useSettingStore from '../../setting/settingstore'
import { renderAIMarkdown } from '../../utils/bookAI'
import { reedyClient } from '../../services/reedy/ReedyClient'
import ReedyMessageCard from './ReedyMessageCard.vue'
import ReedyComposer from './ReedyComposer.vue'
import CitationCard from './CitationCard.vue'
import IndexingStatus from './IndexingStatus.vue'
import SkillChips from './SkillChips.vue'

interface ChatMessage {
  role: 'user' | 'assistant'
  content: string
  citations?: Array<{ cfi: string; chapter: string; text: string }>
}

const props = defineProps<{
  bookHash: string
  bookId: string
  currentPage: number
  currentChapter: number
  chapterTitle: string
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'goto-cfi', cfi: string): void
}>()

const settingStore = useSettingStore()

const messages = ref<ChatMessage[]>([])
const streaming = ref(false)
const answerBuffer = ref('')
const aiMode = ref<'ask' | 'chat'>('ask')
const activeSkillId = ref<string | null>(null)
const skills = ref<Array<{ id: string; name: string; description: string; enabled: boolean }>>([])

async function loadSkills() {
  try {
    skills.value = await reedyClient.listSkills()
  } catch {
    skills.value = []
  }
}
loadSkills()

async function handleSend(text: string) {
  // TODO: Wire into BookReaderModal's askAI flow
  messages.value.push({ role: 'user', content: text })
}

function handleStop() {
  streaming.value = false
}

function handleClear() {
  messages.value = []
}

function handleCopy(content: string) {
  navigator.clipboard.writeText(content)
}

function handleCfiClick(cfi: string) {
  emit('goto-cfi', cfi)
}

function renderContent(content: string): string {
  return renderAIMarkdown(content)
}
</script>

<template>
  <div class="reedy-chat-panel">
    <IndexingStatus
      :book-hash="bookHash"
      :current-page="currentPage"
      @index="() => {}"
      @reindex="() => {}"
      @abort="() => {}"
    />

    <SkillChips
      :skills="skills"
      :active-skill-id="activeSkillId"
      @select="(id) => activeSkillId = id"
    />

    <div class="chat-messages" ref="msgContainer">
      <div v-if="!messages.length" class="empty-state">
        <span>向 AI 助手提问本书内容</span>
        <span class="empty-hint">使用上方技能芯片切换模式</span>
      </div>
      <ReedyMessageCard
        v-for="(msg, idx) in messages"
        :key="idx"
        :role="msg.role"
        :content="renderContent(msg.content)"
        @copy="handleCopy"
        @cfi-click="handleCfiClick"
      />
      <ReedyMessageCard
        v-if="streaming && answerBuffer"
        role="assistant"
        :content="renderContent(answerBuffer)"
        :is-streaming="true"
        @copy="handleCopy"
      />
      <div v-else-if="streaming && !answerBuffer" class="thinking-indicator">
        <span class="dot" />
        <span class="dot" />
        <span class="dot" />
      </div>
    </div>

    <ReedyComposer
      :ai-mode="aiMode"
      :disabled="false"
      :streaming="streaming"
      @send="handleSend"
      @stop="handleStop"
      @toggle-mode="aiMode = aiMode === 'ask' ? 'chat' : 'ask'"
      @clear="handleClear"
    />
  </div>
</template>

<style scoped>
.reedy-chat-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--color-bg-1);
}
.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: 8px 12px;
  display: flex;
  flex-direction: column;
}
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 40px 20px;
  color: var(--color-text-3);
  font-size: 13px;
}
.empty-hint {
  font-size: 11px;
  opacity: 0.6;
}
.thinking-indicator {
  display: flex;
  gap: 4px;
  padding: 8px 12px;
  align-self: flex-start;
}
.dot {
  width: 6px;
  height: 6px;
  background: var(--color-text-3);
  border-radius: 50%;
  animation: dot-blink 1.4s infinite both;
}
.dot:nth-child(2) {
  animation-delay: 0.2s;
}
.dot:nth-child(3) {
  animation-delay: 0.4s;
}
@keyframes dot-blink {
  0% { opacity: 0.2; }
  20% { opacity: 1; }
  100% { opacity: 0.2; }
}
</style>
