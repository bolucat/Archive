<script setup lang='ts'>
import { ref, onMounted, computed } from 'vue'
import useBookLibraryStore from '../store/booklibrary'
import { X, Clock, BookOpen, BarChart3, TrendingUp } from 'lucide-vue-next'
import { getDailyStats, getHeatmapStats, getTotalReadingMinutes, getLongestStreak, getActiveReadingDays } from '../utils/bookReadingRecords'

const emit = defineEmits<{ close: [] }>()
const bookStore = useBookLibraryStore()
const chartTab = ref<'bar' | 'line'>('bar')

interface DayStats { date: string; minutes: number }
interface HeatmapCell { date: string; minutes: number }

const last30Days = ref<DayStats[]>([])
const heatmapData = ref<HeatmapCell[]>([])
const totalReadingMinutes = ref(0)
const longestStreak = ref(0)
const avgDailyMinutes = ref(0)

onMounted(() => {
  // 真实阅读数据
  const daily = getDailyStats(30)
  last30Days.value = daily.map((d) => {
    const date = new Date(d.date)
    return { date: `${date.getMonth() + 1}/${date.getDate()}`, minutes: d.minutes }
  })

  heatmapData.value = getHeatmapStats(52)

  totalReadingMinutes.value = getTotalReadingMinutes()
  longestStreak.value = getLongestStreak()
  const activeDays = getActiveReadingDays()
  avgDailyMinutes.value = activeDays ? Math.round(totalReadingMinutes.value / activeDays) : 0
})

const cards = computed(() => [
  {
    icon: BarChart3,
    value: bookStore.activeBooks.filter(b => (b.reading_progress ?? 0) > 0).length,
    label: 'Books read',
  },
  {
    icon: Clock,
    value: formatTime(totalReadingMinutes.value),
    label: 'Total reading time',
  },
  {
    icon: TrendingUp,
    value: `${longestStreak.value}`,
    label: 'Reading streak (days)',
  },
  {
    icon: BookOpen,
    value: formatTime(avgDailyMinutes.value),
    label: 'Daily average',
  },
])

function formatTime(minutes: number): string {
  if (minutes < 1) return '0m'
  const h = Math.floor(minutes / 60)
  const m = minutes % 60
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}

// Heatmap helpers
function getHeatmapColor(minutes: number): string {
  if (minutes === 0) return 'rgba(128,128,128,0.08)'
  if (minutes < 10) return '#9be9a8'
  if (minutes < 30) return '#40c463'
  if (minutes < 60) return '#30a14e'
  return '#216e39'
}

const weekDays = ['', 'Mon', '', 'Wed', '', 'Fri', '']

// Split heatmap into weeks (columns of 7)
const heatmapWeeks = computed(() => {
  const padded = [...heatmapData.value, ...Array((7 - (heatmapData.value.length % 7 || 7)) % 7).fill(null)]
  const weeks: ({ date: string; minutes: number } | null)[][] = []
  for (let i = 0; i < padded.length; i += 7) {
    weeks.push(padded.slice(i, i + 7))
  }
  return weeks
})

const monthLabels = computed(() => {
  const labels: { label: string; colIndex: number }[] = []
  heatmapWeeks.value.forEach((week, wi) => {
    const cell = wi === 0
      ? week.find(c => c !== null)
      : week.find(c => c && new Date(c.date).getDate() === 1)
    if (cell) {
      const d = new Date(cell.date)
      labels.push({ label: d.toLocaleString('default', { month: 'short' }), colIndex: wi })
    }
  })
  return labels
})

const maxBarVal = computed(() => Math.max(...last30Days.value.map(d => d.minutes), 1))
</script>

<template>
  <div class='stats-page'>
    <!-- Close button (match koodo stats-close-btn) -->
    <div class='stats-close-btn' @click='emit("close")'>
      <X :size='18' :stroke-width='2' />
    </div>

    <!-- Title (match koodo stats-title) -->
    <div class='stats-title'>Reading Stats</div>

    <!-- Cards (match koodo stats-cards: 4-column grid) -->
    <div class='stats-cards'>
      <div v-for='(card, i) in cards' :key='i' class='stats-card'>
        <div class='stats-card-icon'>
          <component :is='card.icon' :size='26' :stroke-width='1.7' />
        </div>
        <div class='stats-card-value'>{{ card.value }}</div>
        <div class='stats-card-label'>{{ card.label }}</div>
      </div>
    </div>

    <!-- Chart Section (match koodo stats-chart-wrapper) -->
    <div class='stats-chart-wrapper'>
      <div class='stats-section-title'>Last 30 Days</div>
      <div class='stats-chart-tabs'>
        <button :class="['stats-chart-tab', chartTab === 'bar' ? 'active' : '']" @click='chartTab = "bar"'>Bar Chart</button>
        <button :class="['stats-chart-tab', chartTab === 'line' ? 'active' : '']" @click='chartTab = "line"'>Line Chart</button>
      </div>

      <!-- Bar Chart (custom SVG) -->
      <div v-if='chartTab === "bar"' class='stats-chart'>
        <svg viewBox='0 0 600 200' preserveAspectRatio='none' style='width:100%;height:200px'>
          <line v-for='i in 4' :key='i' :x1='0' :y1='i * 50' :x2='600' :y2='i * 50' stroke='rgba(128,128,128,0.1)' stroke-dasharray='4,4' />
          <rect
            v-for='(d, idx) in last30Days' :key='idx'
            :x='idx * 20 + 2' :y='200 - (d.minutes / Math.max(maxBarVal, 1) * 180)'
            :width='16' :height='(d.minutes / Math.max(maxBarVal, 1) * 180)'
            rx='3' fill='var(--color-text-2)' opacity='0.7'
          >
            <title>{{ d.date }}: {{ d.minutes }}m</title>
          </rect>
        </svg>
        <div class='stats-chart-xaxis'>
          <span v-for='(d, idx) in last30Days' :key='idx' v-show='idx % 5 === 0'>{{ d.date }}</span>
        </div>
      </div>

      <!-- Line Chart (custom SVG) -->
      <div v-else class='stats-chart'>
        <svg viewBox='0 0 600 200' preserveAspectRatio='none' style='width:100%;height:200px'>
          <defs>
            <linearGradient id='lineGrad' x1='0' y1='0' x2='0' y2='1'>
              <stop offset='0%' stop-color='rgb(var(--primary-6))' stop-opacity='0.32' />
              <stop offset='70%' stop-color='rgb(var(--primary-6))' stop-opacity='0.10' />
              <stop offset='100%' stop-color='rgb(var(--primary-6))' stop-opacity='0.02' />
            </linearGradient>
          </defs>
          <line v-for='i in 4' :key='i' :x1='0' :y1='i * 50' :x2='600' :y2='i * 50' stroke='rgba(128,128,128,0.1)' stroke-dasharray='4,4' />
          <!-- Area fill -->
          <path
            :d="'M ' + last30Days.map((d, idx) => `${idx * 20 + 10},${200 - (d.minutes / Math.max(maxBarVal, 1) * 180)}`).join(' L ') + ' L 600,200 L 0,200 Z'"
            fill='url(#lineGrad)' />
          <!-- Line -->
          <polyline
            :points="last30Days.map((d, idx) => `${idx * 20 + 10},${200 - (d.minutes / Math.max(maxBarVal, 1) * 180)}`).join(' ')"
            fill='none' stroke='rgb(var(--primary-6))' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round' />
        </svg>
        <div class='stats-chart-xaxis'>
          <span v-for='(d, idx) in last30Days' :key='idx' v-show='idx % 5 === 0'>{{ d.date }}</span>
        </div>
      </div>
    </div>

    <!-- Heatmap (match koodo stats-heatmap-wrapper) -->
    <div class='stats-heatmap-wrapper'>
      <div class='stats-section-title'>Reading Activity</div>

      <div class='heatmap-layout'>
        <!-- Left: weekdays -->
        <div class='heatmap-side'>
          <div class='heatmap-month-spacer' />
          <div class='heatmap-weekdays'>
            <div v-for='(day, i) in weekDays' :key='i' class='heatmap-weekday-label'>{{ day }}</div>
          </div>
        </div>

        <!-- Main grid -->
        <div class='heatmap-main'>
          <!-- Month labels -->
          <div class='heatmap-months'>
            <div v-for='(_, wi) in heatmapWeeks' :key='wi' class='heatmap-month-label'>
              {{ monthLabels.find(ml => ml.colIndex === wi)?.label || '' }}
            </div>
          </div>

          <!-- Cells -->
          <div class='heatmap-grid'>
            <div v-for='(week, wi) in heatmapWeeks' :key='wi' class='heatmap-col'>
              <div v-for='(cell, di) in week' :key='di'
                class='heatmap-cell'
                :style='{ backgroundColor: cell ? getHeatmapColor(cell.minutes) : "transparent", visibility: cell === null ? "hidden" : "visible" }'
                :title='cell ? `${cell.date}: ${formatTime(cell.minutes)}` : ""'
              />
            </div>
          </div>
        </div>
      </div>

      <!-- Legend -->
      <div class='heatmap-legend'>
        <span style='margin-right:6px;font-size:11px'>Less</span>
        <div v-for='lvl in [0, 10, 30, 60, 120]' :key='lvl' class='heatmap-legend-cell' :style='{ backgroundColor: getHeatmapColor(lvl) }' />
        <span style='margin-left:6px;font-size:11px'>More</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Match koodo stats.css exactly */
.stats-page {
  position: absolute;
  top: 0; left: 0;
  width: 100%; height: 100%;
  overflow-y: auto;
  z-index: 10;
  box-sizing: border-box;
  padding: 40px 60px 60px 60px;
  background: var(--color-bg-1);
  color: var(--color-text-1);
}

.stats-close-btn {
  position: absolute;
  top: 20px; right: 20px;
  width: 36px; height: 36px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  z-index: 11;
  transition: background 0.2s;
  color: var(--color-text-2);
}
.stats-close-btn:hover { background: var(--color-fill-2); }

.stats-title {
  font-size: 28px;
  font-weight: 700;
  margin-bottom: 32px;
  margin-top: 10px;
}

/* Cards */
.stats-cards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 20px;
  margin-bottom: 40px;
}
@media (max-width: 900px) { .stats-cards { grid-template-columns: repeat(2, 1fr); } .stats-page { padding: 40px 20px 60px 20px; } }

.stats-card {
  border-radius: 16px;
  padding: 24px 20px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: var(--color-bg-2);
}
.stats-card-icon { margin-bottom: 4px; color: var(--color-text-1); }
.stats-card-value { font-size: 25px; font-weight: 700; line-height: 1.1; }
.stats-card-label { font-size: 13px; opacity: 0.6; font-weight: 500; color: var(--color-text-3); }

/* Section */
.stats-section-title { font-size: 17px; font-weight: 600; margin-bottom: 16px; margin-top: 8px; }

/* Chart */
.stats-chart-wrapper { border-radius: 16px; padding: 24px; margin-bottom: 40px; background: var(--color-bg-2); }
.stats-chart-tabs { display: flex; gap: 8px; margin-bottom: 20px; }
.stats-chart-tab {
  padding: 6px 16px; border-radius: 20px; font-size: 13px; font-weight: 500;
  cursor: pointer; border: none;
  transition: background 0.2s, color 0.2s;
  background: rgba(128,128,128,0.08);
  color: var(--color-text-2);
}
.stats-chart-tab.active { background: var(--color-text-2); color: var(--color-bg-1); }
.stats-chart { width: 100%; }
.stats-chart-xaxis {
  display: flex; justify-content: space-between;
  padding: 0 8px; font-size: 10px; opacity: 0.4; margin-top: 4px;
  color: var(--color-text-3);
}

/* Heatmap */
.stats-heatmap-wrapper {
  --heatmap-cell-size: 13px; --heatmap-gap: 3px; --heatmap-side-width: 34px;
  border-radius: 16px; padding: 24px; margin-bottom: 40px;
  overflow-x: auto; min-width: 0; background: var(--color-bg-2);
}
.heatmap-layout { display: flex; align-items: flex-start; gap: 8px; width: 100%; min-width: 400px; }
.heatmap-side { width: var(--heatmap-side-width); flex-shrink: 0; display: flex; flex-direction: column; align-self: stretch; }
.heatmap-main { display: flex; flex-direction: column; gap: 6px; flex: 1; min-width: 0; }
.heatmap-month-spacer { height: 17px; }
.heatmap-grid { display: flex; gap: var(--heatmap-gap); width: 100%; }
.heatmap-col { display: flex; flex-direction: column; gap: var(--heatmap-gap); flex: 1; min-width: 0; }
.heatmap-cell { width: 100%; aspect-ratio: 1/1; border-radius: 3px; cursor: default; }
.heatmap-months { display: flex; gap: var(--heatmap-gap); width: 100%; }
.heatmap-month-label {
  font-size: 11px; opacity: 0.55; flex: 1; min-width: 0;
  white-space: nowrap; overflow: visible; color: var(--color-text-3);
}
.heatmap-weekdays { display: flex; flex-direction: column; gap: var(--heatmap-gap); justify-content: flex-start; flex: 1; }
.heatmap-weekday-label { font-size: 11px; opacity: 0.55; flex: 1; display: flex; align-items: center; color: var(--color-text-3); }
.heatmap-legend { display: flex; align-items: center; gap: 4px; margin-top: 10px; font-size: 11px; opacity: 0.55; justify-content: flex-end; color: var(--color-text-3); }
.heatmap-legend-cell { width: 13px; height: 13px; border-radius: 3px; }
</style>
