<script setup lang="ts">
import { BellRing, CheckCircle2, CircleDotDashed, RefreshCw, SearchCheck, X } from 'lucide-vue-next'
import { onBeforeUnmount, onMounted, ref } from 'vue'
import message from '../../utils/message'
import { endMediaAcquisitionTracking, listMediaAcquisitionTracking } from '../../services/mediaAcquisition/client'
import { runMediaAcquisitionTrackingPatrol } from '../../services/mediaAcquisition/workflowRunner'
import type { MediaAcquisitionTrackingItem } from '@shared/types/mediaAcquisition'

const items = ref<MediaAcquisitionTrackingItem[]>([])
const loading = ref(false)
const patrolling = ref(false)
const endingId = ref('')
let timer: ReturnType<typeof setInterval> | undefined

async function refresh() {
  loading.value = true
  try {
    items.value = (await listMediaAcquisitionTracking(100)).filter(item => item.status !== 'ended')
  } catch (error: any) {
    message.error(error?.message || '读取追更状态失败')
  } finally {
    loading.value = false
  }
}

async function patrol(trackingId?: string) {
  patrolling.value = true
  try {
    await runMediaAcquisitionTrackingPatrol({ force: true, trackingId })
    await refresh()
    message.success(trackingId ? '本季巡检完成' : '全部追更巡检完成')
  } catch (error: any) {
    message.error(error?.message || '追更巡检失败')
  } finally {
    patrolling.value = false
  }
}

async function endTracking(id: string) {
  endingId.value = id
  try {
    await endMediaAcquisitionTracking(id)
    await refresh()
    message.success('已取消追更，网盘文件不会删除')
  } catch (error: any) {
    message.error(error?.message || '取消追更失败')
  } finally {
    endingId.value = ''
  }
}

function checkTime(value?: number) {
  return value ? new Date(value).toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }) : '等待首次巡检'
}

onMounted(() => {
  void refresh()
  timer = setInterval(() => void refresh(), 10_000)
})
onBeforeUnmount(() => timer && clearInterval(timer))
</script>

<template>
  <section class="tracking-page">
    <header class="tracking-head">
      <div><span>MEDIA ACQUISITION</span><h2>追更</h2><p>只补全已经播出但尚未入库的剧集，未来集不会计入缺集。</p></div>
      <div class="tracking-tools"><button type="button" :disabled="patrolling" title="立即巡检全部追更" @click="patrol()"><SearchCheck :size="16" :class="{ spin: patrolling }" /></button><button type="button" :disabled="loading" title="刷新追更状态" @click="refresh"><RefreshCw :size="16" :class="{ spin: loading }" /></button></div>
    </header>
    <div v-if="!items.length && !loading" class="tracking-empty"><BellRing :size="26" /><strong>还没有追更任务</strong><span>为电视剧创建获取任务后，会自动显示在这里。</span></div>
    <div v-else class="tracking-list">
      <article v-for="item in items" :key="item.id" class="tracking-card">
        <CheckCircle2 v-if="item.status === 'complete'" :size="22" class="complete" />
        <CircleDotDashed v-else :size="22" class="active" />
        <div class="tracking-main"><strong>{{ item.title }} · 第 {{ item.seasonNumber }} 季</strong><p>获取 / 已播 / 总集：{{ item.obtainedEpisodes }}/{{ item.latestAiredEpisode || '?' }}/{{ item.totalEpisodes || '?' }} <template v-if="item.missingEpisodes.length">· 缺 E{{ item.missingEpisodes.join('、E') }}</template><template v-else-if="item.status === 'tracking'">· 已追至最新</template></p><small>下次巡检：{{ checkTime(item.nextCheckAt) }}</small></div>
        <div class="tracking-actions"><span :class="['tracking-status', item.status]">{{ item.status === 'complete' ? '本季完整' : item.missingEpisodes.length ? '等待补全' : '追更中' }}</span><button type="button" title="立即巡检本季" :disabled="patrolling" @click="patrol(item.id)"><SearchCheck :size="14" /></button><a-popconfirm content="取消追更不会删除网盘文件，确定继续？" @ok="endTracking(item.id)"><button type="button" title="取消追更" :disabled="endingId === item.id"><X :size="14" /></button></a-popconfirm></div>
      </article>
    </div>
  </section>
</template>

<style scoped>
.tracking-page { height: 100%; overflow: auto; padding: 32px; color: var(--color-text-1); background: var(--color-bg-1); }.tracking-head { display: flex; justify-content: space-between; gap: 24px; padding-bottom: 24px; border-bottom: 1px solid var(--color-border-2); }.tracking-head span { color: rgb(var(--primary-6)); font-size: 11px; font-weight: 700; letter-spacing: 1.2px; }.tracking-head h2 { margin: 5px 0; font-size: 25px; }.tracking-head p { margin: 0; color: var(--color-text-3); }.tracking-head button, .tracking-actions button { display: inline-flex; align-items: center; justify-content: center; width: 34px; height: 34px; border: 1px solid var(--color-border-2); border-radius: 6px; color: var(--color-text-2); background: transparent; cursor: pointer; }.tracking-tools, .tracking-actions { display: flex; align-items: center; gap: 7px; }.tracking-actions button { width: 28px; height: 28px; }.tracking-empty { min-height: 280px; display: grid; place-content: center; justify-items: center; gap: 8px; color: var(--color-text-3); }.tracking-empty strong { color: var(--color-text-1); }.tracking-list { display: grid; gap: 9px; max-width: 980px; padding-top: 22px; }.tracking-card { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 12px; padding: 15px 16px; border: 1px solid var(--color-border-2); border-radius: 7px; background: var(--color-bg-2); }.tracking-main strong { display: block; }.tracking-main p, .tracking-main small { display: block; margin: 5px 0 0; color: var(--color-text-3); font-size: 12px; }.tracking-main small { font-size: 11px; }.tracking-status { padding: 4px 8px; border-radius: 99px; color: rgb(var(--primary-6)); background: rgb(var(--primary-1)); font-size: 12px; white-space: nowrap; }.tracking-status.complete { color: rgb(var(--success-6)); background: rgb(var(--success-1)); }.active { color: rgb(var(--primary-6)); }.complete { color: rgb(var(--success-6)); }.spin { animation: tracking-spin .8s linear infinite; }@keyframes tracking-spin { to { transform: rotate(360deg); } }
</style>
