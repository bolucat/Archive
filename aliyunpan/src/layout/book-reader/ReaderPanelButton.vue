<script setup lang="ts">
defineProps<{
  title: string
  active?: boolean
  disabled?: boolean
}>()

const emit = defineEmits<{ (e: 'click', event: MouseEvent): void }>()
</script>

<template>
  <button
    :title="title"
    :disabled="disabled"
    :class="['reader-panel-button', active ? 'active' : '']"
    @click="emit('click', $event)"
  >
    <slot />
  </button>
</template>

<style scoped>
.reader-panel-button {
  width: 34px;
  height: 34px;
  border: 0;
  border-radius: 6px;
  display: grid;
  place-items: center;
  background: transparent;
  color: var(--reader-text, var(--color-text-2));
  cursor: pointer;
}

.reader-panel-button:hover,
.reader-panel-button.active {
  background: rgba(127, 127, 127, .14);
}

.reader-panel-button:disabled {
  cursor: default;
  opacity: .42;
}
</style>
