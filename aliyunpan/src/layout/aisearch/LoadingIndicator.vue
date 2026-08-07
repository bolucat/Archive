<script setup lang="ts">
import { Sparkles } from 'lucide-vue-next'
import ThinkingOrb from './ThinkingOrb.vue'
</script>

<template>
  <div class="ai-loading">
    <ThinkingOrb class="ai-loading-orb" />
    <Sparkles :size="14" :stroke-width="2" class="ai-loading-icon" />
  </div>
</template>

<style scoped>
.ai-loading { display: flex; align-items: center; gap: 7px; padding: 2px 0; }
.ai-loading-orb { transform: scale(.58); margin: -6px; }
.ai-loading-icon { color: rgb(var(--primary-6)); opacity: .75; }
</style>
