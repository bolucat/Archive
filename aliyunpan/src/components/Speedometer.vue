<script setup lang='ts'>
import { computed } from 'vue'
import { useDowningStore } from '../store'

const downingStore = useDowningStore()

const totalDownSpeed = computed(() => {
  let total = 0
  for (const item of downingStore.ListDataRaw) {
    if (item.Down.IsDowning) total += item.Down.DownSpeed || 0
  }
  return total
})

const formatSpeed = (bytes: number): string => {
  if (bytes >= 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB/s'
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB/s'
  return bytes + ' B/s'
}

const speedText = computed(() => {
  if (!totalDownSpeed.value) return '空闲'
  return formatSpeed(totalDownSpeed.value)
})

const isActive = computed(() => totalDownSpeed.value > 0)
</script>

<template>
  <div class='speedometer' :class="{ active: isActive }">
    <IconFont name='iconcloud-download' class='speed-icon' />
    <span class='speed-text'>{{ speedText }}</span>
  </div>
</template>

<style scoped>
.speedometer {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--color-text-3);
  padding: 0 6px;
  border-radius: 4px;
  height: 22px;
  transition: color 0.2s;
}
.speedometer.active {
  color: var(--color-success-6, #00b42a);
}
.speed-icon {
  font-size: 13px;
}
.speed-text {
  min-width: 70px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
</style>
