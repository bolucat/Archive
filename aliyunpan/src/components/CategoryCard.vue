<template>
  <div
    class="category-card"
    :class="[`category-card--${type}`, { 'category-card--has-collage': collageUrls.length > 0 }]"
    @click="$emit('click', { name, type, count })"
    @contextmenu.prevent="$emit('contextmenu', $event, { name, type, count })"
  >
    <!-- 封面拼贴图层：2×2 迷你海报 -->
    <div v-if="collageUrls.length > 0" class="category-card__collage">
      <div
        v-for="(url, idx) in collageUrls"
        :key="idx"
        class="category-card__collage-cell"
      >
        <img :src="url" :alt="`${name} poster ${idx + 1}`" loading="lazy" />
      </div>
      <div class="category-card__collage-overlay" />
    </div>

    <!-- 无海报时的渐变背景 -->
    <div v-else class="category-card__gradient-bg">
      <div class="category-card__gradient-icon">
        <IconFont :name="typeIcon" />
      </div>
    </div>

    <!-- 底部信息区 -->
    <div class="category-card__info">
      <div class="category-card__label">
        <IconFont :name="typeIcon" class="category-card__label-icon" />
        <span class="category-card__name">{{ name }}</span>
      </div>
      <div class="category-card__count-badge">
        <span class="category-card__count-num">{{ count }}</span>
        <span class="category-card__count-unit">项</span>
      </div>
    </div>

    <!-- 类型标记 -->
    <div class="category-card__type-chip">
      {{ typeLabel }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

interface Props {
  name: string
  count: number
  type: 'genre' | 'rating' | 'year' | 'playlist'
  gradient?: string
  coverImage?: string
  coverImages?: string[]
}

const props = withDefaults(defineProps<Props>(), {
  gradient: undefined,
  coverImage: undefined,
  coverImages: () => []
})

defineEmits<{
  (e: 'click', data: { name: string; type: string; count: number }): void
  (e: 'contextmenu', event: MouseEvent, data: { name: string; type: string; count: number }): void
}>()

const collageUrls = computed(() => {
  if (props.coverImages && props.coverImages.length > 0) {
    return props.coverImages.slice(0, 4)
  }
  if (props.coverImage) {
    return [props.coverImage]
  }
  return []
})

const typeIcon = computed(() => {
  switch (props.type) {
    case 'genre': return 'iconwbiaoqian'
    case 'rating': return 'iconcrown2'
    case 'year': return 'iconcalendar'
    case 'playlist': return 'iconlist'
    default: return 'iconwbiaoqian'
  }
})

const typeLabel = computed(() => {
  switch (props.type) {
    case 'genre': return '类型'
    case 'rating': return '评分'
    case 'year': return '年份'
    case 'playlist': return '列表'
    default: return ''
  }
})
</script>

<style scoped>
.category-card {
  position: relative;
  width: 100%;
  aspect-ratio: 16 / 10;
  min-height: 160px;
  border-radius: 18px;
  cursor: pointer;
  overflow: hidden;
  transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1), box-shadow 0.3s ease;
  box-shadow: 0 12px 32px rgba(12, 14, 18, 0.12);
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: #14171c;
}

.category-card:hover {
  transform: translateY(-3px);
  box-shadow: 0 20px 44px rgba(12, 14, 18, 0.22);
}

/* ---- 拼贴图层 ---- */
.category-card__collage {
  position: absolute;
  inset: 0;
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-template-rows: 1fr 1fr;
  gap: 1px;
}

.category-card__collage-cell {
  overflow: hidden;
  background: #1a1d24;
}

.category-card__collage-cell img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}

.category-card:hover .category-card__collage-cell img {
  transform: scale(1.06);
}

.category-card__collage-overlay {
  position: absolute;
  inset: 0;
  background: linear-gradient(
    0deg,
    rgba(12, 14, 18, 0.78) 0%,
    rgba(12, 14, 18, 0.32) 48%,
    rgba(12, 14, 18, 0.1) 100%
  );
  pointer-events: none;
}

/* ---- 渐变背景（无海报回退） ---- */
.category-card__gradient-bg {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.category-card--genre .category-card__gradient-bg {
  background: linear-gradient(135deg, #0d3b2e 0%, #142b1f 50%, #0f2020 100%);
}

.category-card--rating .category-card__gradient-bg {
  background: linear-gradient(135deg, #3d2910 0%, #2b200c 50%, #1a1a0f 100%);
}

.category-card--year .category-card__gradient-bg {
  background: linear-gradient(135deg, #1c2028 0%, #181c24 50%, #14161c 100%);
}

.category-card--playlist .category-card__gradient-bg {
  background: linear-gradient(135deg, #1a1728 0%, #1f1a2e 50%, #181420 100%);
}

.category-card__gradient-icon {
  width: 56px;
  height: 56px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.32);
  font-size: 28px;
}

/* ---- 底部信息区 ---- */
.category-card__info {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 56px 16px 14px 16px;
  z-index: 2;
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 8px;
}

.category-card__label {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
  flex: 1;
}

.category-card__label-icon {
  flex-shrink: 0;
  font-size: 14px;
  color: rgba(255, 255, 255, 0.52);
}

.category-card__name {
  font-size: 15px;
  font-weight: 700;
  color: rgba(255, 255, 255, 0.94);
  line-height: 1.25;
  text-shadow: 0 2px 10px rgba(0, 0, 0, 0.5);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---- 数量徽章 ---- */
.category-card__count-badge {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 4px 10px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.category-card__count-num {
  font-size: 13px;
  font-weight: 700;
  color: rgba(255, 255, 255, 0.9);
  line-height: 1;
}

.category-card__count-unit {
  font-size: 11px;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.48);
}

/* ---- 类型标记 ---- */
.category-card__type-chip {
  position: absolute;
  top: 10px;
  left: 10px;
  z-index: 3;
  padding: 3px 9px;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.42);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  font-size: 11px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.56);
  letter-spacing: 0.3px;
  transition: color 0.25s ease, background-color 0.25s ease;
}

.category-card--genre .category-card__type-chip {
  color: rgba(74, 222, 128, 0.72);
}

.category-card--rating .category-card__type-chip {
  color: rgba(251, 191, 36, 0.72);
}

.category-card--year .category-card__type-chip {
  color: rgba(148, 163, 184, 0.72);
}

.category-card--playlist .category-card__type-chip {
  color: rgba(167, 139, 250, 0.72);
}

/* ---- 深色模式 ---- */
[arco-theme='dark'] .category-card {
  border-color: rgba(255, 255, 255, 0.06);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.32);
}

[arco-theme='dark'] .category-card:hover {
  box-shadow: 0 20px 44px rgba(0, 0, 0, 0.42);
}

/* ---- 响应式 ---- */
@media (max-width: 768px) {
  .category-card {
    min-height: 140px;
    border-radius: 16px;
  }

  .category-card__info {
    padding: 48px 12px 12px 12px;
  }

  .category-card__name {
    font-size: 14px;
  }

  .category-card__gradient-icon {
    width: 44px;
    height: 44px;
    font-size: 22px;
  }
}
</style>
