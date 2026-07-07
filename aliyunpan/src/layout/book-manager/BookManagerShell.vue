<script setup lang="ts">
defineProps<{ dragActive?: boolean }>()

const emit = defineEmits<{
  (e: 'dropFiles', files: File[]): void
  (e: 'dragState', value: boolean): void
}>()

function handleDrop(event: DragEvent) {
  emit('dropFiles', Array.from(event.dataTransfer?.files || []))
  emit('dragState', false)
}
</script>

<template>
  <section
    class="book-manager-shell"
    @dragover.prevent="emit('dragState', true)"
    @dragleave.prevent="emit('dragState', false)"
    @drop.prevent="handleDrop"
  >
    <aside class="book-manager-sidebar-slot">
      <slot name="sidebar" />
    </aside>
    <section class="book-manager-body">
      <header class="book-manager-header-slot">
        <slot name="header" />
      </header>
      <main class="book-manager-main-slot">
        <slot />
      </main>
      <footer class="book-manager-footer-slot">
        <slot name="footer" />
      </footer>
    </section>
    <div :class="['book-manager-drop-overlay', dragActive ? 'active' : '']">
      <span>Drop books to import</span>
    </div>
  </section>
</template>

<style scoped>
.book-manager-shell {
  width: 100%;
  height: 100%;
  min-height: 0;
  position: relative;
}

.book-manager-drop-overlay {
  pointer-events: none;
  position: absolute;
  inset: 0;
  display: none;
  place-items: center;
  background: rgba(0, 0, 0, .22);
  z-index: 30;
}

.book-manager-drop-overlay.active {
  display: grid;
  pointer-events: auto;
}

.book-manager-drop-overlay span {
  padding: 14px 22px;
  border-radius: 8px;
  background: var(--color-bg-1);
  box-shadow: 0 12px 40px rgba(0, 0, 0, .22);
}

.book-manager-sidebar-slot,
.book-manager-header-slot,
.book-manager-main-slot,
.book-manager-footer-slot {
  min-width: 0;
  min-height: 0;
}

.book-manager-sidebar-slot,
.book-manager-header-slot,
.book-manager-footer-slot {
  display: contents;
}

.book-manager-body,
.book-manager-main-slot {
  width: 100%;
  height: 100%;
}
</style>
