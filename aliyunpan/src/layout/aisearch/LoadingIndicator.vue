<script setup lang="ts">
import { Sparkles } from 'lucide-vue-next'
</script>

<template>
  <div class="ai-loading">
    <Sparkles :size="14" :stroke-width="2" class="ai-loading-icon" />
    <span class="ai-loading-cursor">▊</span>
  </div>
</template>

<style scoped>
.ai-loading { display: flex; align-items: center; gap: 6px; padding: 6px 0; }
.ai-loading-icon { color: rgb(var(--primary-6)); opacity: 0.5; }
.ai-loading-cursor { font-size: 16px; color: rgb(var(--primary-6)); animation: ai-blink 0.8s step-end infinite; }
@keyframes ai-blink { 50% { opacity: 0; } }
</style>
