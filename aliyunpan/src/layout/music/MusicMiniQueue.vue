<script setup lang="ts">
import { computed } from 'vue'
import { X } from 'lucide-vue-next'
import type { IPageMusicTrack } from '../../store/appstore'
import { t as tt } from '../../i18n'

const props = defineProps<{
  visible: boolean
  tracks: IPageMusicTrack[]
  currentIndex: number
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'play', index: number): void
  (e: 'remove', index: number): void
}>()

const displayList = computed(() => {
  const start = Math.max(0, props.currentIndex - 3)
  const end = Math.min(props.tracks.length, props.currentIndex + 8)
  return props.tracks.slice(start, end).map((t, i) => ({ track: t, globalIndex: start + i }))
})
</script>

<template>
  <div v-if="visible" :class="['mini-queue-pop', 'mini-queue-popover', visible ? 'show' : '']" @click.stop>
    <div class="mini-queue-head">
      <span class="mini-queue-title">{{ tt('music.currentQueue') }}</span>
      <span class="mini-queue-count">{{ tracks.length }} {{ tt('music.tracksUnit') }}</span>
      <button class="mini-queue-close" @click="emit('close')"><X :size="14" /></button>
    </div>
    <div class="mini-queue-list">
      <div
        v-for="item in displayList"
        :key="item.track.file_id + item.globalIndex"
        :class="['mini-queue-row', item.globalIndex === currentIndex ? 'now' : '']"
        @click="emit('play', item.globalIndex)"
      >
        <span class="mini-queue-idx">{{ item.globalIndex + 1 }}</span>
        <span class="mini-queue-name">{{ item.track.file_name }}</span>
        <button class="mini-queue-rm" @click.stop="emit('remove', item.globalIndex)" :title="tt('music.remove')">×</button>
      </div>
      <div v-if="!tracks.length" class="mini-queue-empty">{{ tt('music.queueEmpty') }}</div>
    </div>
  </div>
</template>

<style scoped lang="less">
.mini-queue-pop {
  position: absolute;
  bottom: 118px;
  left: 50%;
  transform: translateX(-50%) translateY(8px) scale(.96);
  width: min(400px, calc(100vw - 32px));
  max-height: 360px;
  padding: 12px;
  border-radius: 14px;
  border: 1px solid rgba(0,245,212,.14);
  background: linear-gradient(145deg, rgba(18,18,23,.88), rgba(8,8,12,.94));
  backdrop-filter: blur(28px) saturate(1.1);
  box-shadow: 0 22px 64px rgba(0,0,0,.42), inset 0 1px 0 rgba(255,255,255,.06);
  opacity: 0;
  pointer-events: none;
  transition: opacity .2s, transform .2s;
  z-index: 12;
  overflow: hidden;
}
.mini-queue-pop.show { opacity: 1; pointer-events: auto; transform: translateX(-50%) translateY(0) scale(1); }
.mini-queue-head {
  display: flex; align-items: center; gap: 10px; margin-bottom: 8px;
}
.mini-queue-title { font-size: 12px; font-weight: 700; color: rgba(255,255,255,.9); }
.mini-queue-count { font-size: 10.5px; color: rgba(255,255,255,.4); flex: 1; }
.mini-queue-close {
  width: 24px; height: 24px; border: 1px solid rgba(255,255,255,.1); border-radius: 6px;
  background: rgba(255,255,255,.04); color: rgba(255,255,255,.5); cursor: pointer;
  display: flex; align-items: center; justify-content: center;
}
.mini-queue-close:hover { background: rgba(255,255,255,.1); color: #fff; }
.mini-queue-list { display: flex; flex-direction: column; gap: 4px; max-height: 280px; overflow-y: auto; }
.mini-queue-row {
  display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 8px;
  border: 1px solid rgba(255,255,255,.04); background: rgba(255,255,255,.025); cursor: pointer;
  transition: all .15s;
}
.mini-queue-row:hover { background: rgba(0,245,212,.07); border-color: rgba(0,245,212,.18); }
.mini-queue-row.now { border-color: rgba(0,245,212,.32); background: rgba(0,245,212,.075); }
.mini-queue-idx { font-size: 10.5px; color: rgba(255,255,255,.34); width: 20px; text-align: right; }
.mini-queue-name { flex: 1; font-size: 11.5px; color: rgba(255,255,255,.85); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.mini-queue-rm { width: 22px; height: 22px; border: none; border-radius: 5px; background: rgba(255,255,255,.06); color: rgba(255,255,255,.45); cursor: pointer; font-size: 14px; }
.mini-queue-rm:hover { background: rgba(255,86,100,.7); color: #fff; }
.mini-queue-empty { padding: 20px; text-align: center; color: rgba(255,255,255,.32); font-size: 11px; }
</style>
