<script setup lang="ts">
defineProps<{
  pageColor: string
}>()
</script>

<template>
  <div class="reader-bg" :style="{ backgroundColor: pageColor }" />
</template>

<style scoped>
.reader-bg {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  width: 100%;
  box-sizing: border-box;
  z-index: -10;
  pointer-events: none;
}
</style>
