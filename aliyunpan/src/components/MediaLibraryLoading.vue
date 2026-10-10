<script setup lang="ts">
import MediaLoadingIndicator from './MediaLoadingIndicator.vue'
</script>
<template><div class="media-library-loading" aria-busy="true"><MediaLoadingIndicator /></div></template>
<style scoped>
.media-library-loading{display:grid;place-items:center;width:100%;height:100%;min-height:56vh;background:var(--color-bg-1)}
</style>
