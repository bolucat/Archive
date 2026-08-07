<script setup lang="ts">
withDefaults(defineProps<{ active?: boolean }>(), { active: false })
</script>

<template>
  <div class="border-beam" :class="{ 'border-beam--active': active }">
    <slot />
  </div>
</template>

<style scoped>
.border-beam { position: relative; isolation: isolate; }
.border-beam::after { position: absolute; z-index: 2; inset: -1px; padding: 1px; pointer-events: none; content: ''; border-radius: inherit; background: conic-gradient(from 215deg, transparent 0 14%, rgba(var(--primary-5), .18) 21%, transparent 29% 56%, rgba(77, 218, 198, .14) 66%, transparent 74%); opacity: .38; mask: linear-gradient(#000 0 0) content-box exclude, linear-gradient(#000 0 0); }
.border-beam--active::after { opacity: 1; background: conic-gradient(from var(--beam-angle), transparent 0 11%, #9d7cff 16%, #6bc9ff 20%, transparent 26% 53%, #46e0be 60%, #caa8ff 65%, transparent 71%); filter: drop-shadow(0 0 6px rgba(var(--primary-5), .65)); animation: border-beam-orbit 4.8s linear infinite; }

@property --beam-angle { syntax: '<angle>'; inherits: false; initial-value: 0deg; }
@keyframes border-beam-orbit { to { --beam-angle: 360deg; } }
@media (prefers-reduced-motion: reduce) { .border-beam--active::after { animation: none; } }
</style>
