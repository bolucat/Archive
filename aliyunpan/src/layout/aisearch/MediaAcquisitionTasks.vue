<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { CircleAlert, Clock3, Download, LoaderCircle, RefreshCw, Trash2, X } from 'lucide-vue-next'
import message from '../../utils/message'
import { addMediaAcquisitionCandidate, addMediaAcquisitionEvent, clearCompletedMediaAcquisitionRuns, forceCancelMediaAcquisitionRun, listMediaAcquisitionRuns, selectMediaAcquisitionCandidate } from '../../services/mediaAcquisition/client'
import type { MediaAcquisitionRunView } from '@shared/types/mediaAcquisition'
import { createMediaAcquisitionCandidateInput } from '../../services/mediaAcquisition/shareExecutor'
import { runMediaAcquisitionWorkflow } from '../../services/mediaAcquisition/workflowRunner'
import { trashVerifiedMediaAcquisitionDuplicates } from '../../services/mediaAcquisition/duplicateCleanup'
import type { MediaAcquisitionDuplicateGroup } from '../../services/mediaAcquisition/duplicatePolicy'

const runs = ref<MediaAcquisitionRunView[]>([])
const loading = ref(false)
const pendingLinks = ref<Record<string, string>>({})
const pendingPasswords = ref<Record<string, string>>({})
const executingCandidateId = ref('')
const cleaningRunId = ref('')
const clearingCompleted = ref(false)
let timer: ReturnType<typeof setInterval> | undefined

const processingRuns = computed(() => runs.value.filter(run => ['searching', 'selecting', 'transferring', 'verifying', 'organizing'].includes(run.status)))
const queuedRuns = computed(() => runs.value.filter(run => ['reserved', 'queued', 'retry_wait'].includes(run.status)))
const completedRuns = computed(() => runs.value.filter(run => ['completed', 'partial', 'no_coverage', 'failed', 'cancelled'].includes(run.status)))
const sourceRecoverableStatuses = new Set<MediaAcquisitionRunView['status']>(['partial', 'no_coverage', 'failed'])
const endableStatuses = new Set<MediaAcquisitionRunView['status']>(['reserved', 'queued', 'searching', 'selecting', 'transferring', 'verifying', 'organizing', 'retry_wait'])

async function refresh() {
  loading.value = true
  try {
    runs.value = await listMediaAcquisitionRuns(60)
  } catch (error: any) {
    message.error(error?.message || '读取媒体任务失败')
  } finally {
    loading.value = false
  }
}

async function cancel(run: MediaAcquisitionRunView) {
  const result = await forceCancelMediaAcquisitionRun(run.id)
  await refresh()
  const current = runs.value.find(item => item.id === run.id) || result
  if (current?.status === 'cancelled') {
    message.success('任务已强制结束')
    return
  }
  if (current && completedRuns.value.some(item => item.id === current.id)) {
    message.info(`任务已${current.status === 'completed' ? '完成' : '进入结束状态'}`)
    return
  }
  message.warning('任务状态仍在处理中，请稍后重试')
}

function endActionLabel(status: MediaAcquisitionRunView['status']) {
  return ['transferring', 'verifying', 'organizing'].includes(status) ? '强制结束' : '取消'
}

async function clearCompleted() {
  if (!completedRuns.value.length) return
  clearingCompleted.value = true
  try {
    const count = await clearCompletedMediaAcquisitionRuns()
    message.success(`已清理 ${count} 条已结束任务`)
    await refresh()
  } catch (error: any) {
    message.error(error?.message || '清理已结束任务失败')
  } finally {
    clearingCompleted.value = false
  }
}

async function addShare(run: MediaAcquisitionRunView) {
  const input = createMediaAcquisitionCandidateInput(pendingLinks.value[run.id] || '', pendingPasswords.value[run.id] || '', run.target.title)
  if (!input) {
    message.warning('请输入分享链接、磁力或 HTTP/HTTPS 链接')
    return
  }
  try {
    await addMediaAcquisitionCandidate(run.id, input)
    void runMediaAcquisitionWorkflow(run.id)
    pendingLinks.value[run.id] = ''
    pendingPasswords.value[run.id] = ''
    await refresh()
  } catch (error: any) {
    message.error(error?.message || '添加候选资源失败')
  }
}

async function executeCandidate(run: MediaAcquisitionRunView, candidateId: string) {
  executingCandidateId.value = candidateId
  try {
    const selected = await selectMediaAcquisitionCandidate(run.id, candidateId, '用户手动确认执行该候选')
    if (!selected?.candidates.some(candidate => candidate.id === candidateId && candidate.status === 'selected')) {
      message.warning('该候选已被其他任务执行器接管')
      return
    }
    void runMediaAcquisitionWorkflow(run.id)
    message.success('已交给 Agent 执行转存')
  } catch (error: any) {
    message.error(error?.message || '分享导入失败')
  } finally {
    executingCandidateId.value = ''
    await refresh()
  }
}

function statusLabel(status: MediaAcquisitionRunView['status']) {
  return ({ reserved: '已预定', queued: '排队中', searching: '搜索中', selecting: 'Agent 挑选资源', transferring: '转存中', verifying: '核对入库目录', organizing: '扫描媒体库', retry_wait: '等待重试', completed: '已完成', partial: '部分完成', no_coverage: '暂无资源', failed: '失败', cancelled: '已取消' } as Record<string, string>)[status] || status
}

function phaseLabel(phase: string) {
  return ({ queued: '排队', search: '搜索', select: '筛选', transfer: '转存', verify: '核验', organize: '整理', finalize: '完成' } as Record<string, string>)[phase] || phase
}

function formatTime(time: number) {
  return new Date(time).toLocaleString('zh-CN', { hour: '2-digit', minute: '2-digit', month: 'numeric', day: 'numeric' })
}

function formatDuration(ms: number) {
  const seconds = Math.max(0, Math.ceil(ms / 1000))
  if (seconds < 60) return `${seconds} 秒`
  const minutes = Math.ceil(seconds / 60)
  if (minutes < 60) return `${minutes} 分钟`
  const hours = Math.floor(minutes / 60)
  const restMinutes = minutes % 60
  return restMinutes ? `${hours} 小时 ${restMinutes} 分钟` : `${hours} 小时`
}

function retryHint(run: MediaAcquisitionRunView) {
  if (!['reserved', 'retry_wait', 'queued'].includes(run.status) || !run.nextAttemptAt) return ''
  if (run.status === 'reserved') return `将于 ${formatTime(run.nextAttemptAt)} 上映后自动开始获取`
  const remaining = run.nextAttemptAt - Date.now()
  if (run.status === 'queued' && run.searchAttemptCount) return remaining > 0 ? `第 ${run.searchAttemptCount} 次搜索重试将在 ${formatDuration(remaining)} 后开始` : '搜索重试时间已到，等待调度器接管'
  return remaining > 0 ? `第 ${run.attemptCount} 次重试将在 ${formatDuration(remaining)} 后开始` : '重试时间已到，等待调度器接管'
}

function candidateStatusLabel(candidate: MediaAcquisitionRunView['candidates'][number]) {
  return ({ selected: 'Agent 已选中', imported: '等待网盘完成', transferring: '转存中', failed: '失败', rejected: '已跳过', pending: '待选择' } as Record<string, string>)[candidate.status] || candidate.status
}

function candidateHint(run: MediaAcquisitionRunView, candidate: MediaAcquisitionRunView['candidates'][number]) {
  if (candidate.lastError) return candidate.lastError
  const decision = [...(run.decisions || [])].reverse().find(item => item.candidateId === candidate.id && (item.decision === 'select' || item.decision === 'reject'))
  if (decision?.reason) {
    const normalize = (value: string) => value.replace(/[\s\u3000]+/g, '').toLowerCase()
    const targetTitle = normalize(run.target.title)
    // Older runs may contain an Agent-generated reason copied from another title.
    // Do not expose it as if it were evidence for this task.
    if (targetTitle.length >= 2 && !normalize(decision.reason).includes(targetTitle)) return `候选「${candidate.title}」与目标「${run.target.title}」匹配，详情以实际落盘核验为准。`
    return decision.reason
  }
  if (candidate.status === 'rejected') return 'Agent 已选择更匹配的候选，此资源暂不执行。'
  return ''
}

function eventMessage(run: MediaAcquisitionRunView, event: MediaAcquisitionRunView['events'][number]) {
  const candidateId = typeof event.data?.candidateId === 'string' ? event.data.candidateId : undefined
  if (event.phase === 'select' && event.data?.tool === 'transferCandidate' && candidateId && event.message.startsWith('Agent 决策：')) {
    const candidate = run.candidates.find(item => item.id === candidateId)
    if (candidate) return `Agent 决策：候选「${candidate.title}」与目标「${run.target.title}」匹配，详情以实际落盘核验为准。`
  }
  return event.message
}

function duplicateGroups(run: MediaAcquisitionRunView): MediaAcquisitionDuplicateGroup[] {
  const event = [...run.events].reverse().find(item => item.data?.tool === 'duplicateEpisodeAudit')
  const groups = event?.data?.groups
  return Array.isArray(groups) ? groups as MediaAcquisitionDuplicateGroup[] : []
}

async function cleanDuplicates(run: MediaAcquisitionRunView) {
  const groups = duplicateGroups(run)
  const count = groups.reduce((total, group) => total + group.deleteCandidates.length, 0)
  if (!count) return
  if (!window.confirm(`将把 ${count} 个较小重复集移入网盘回收站，保留每集较大的文件。是否继续？`)) return
  cleaningRunId.value = run.id
  try {
    const result = await trashVerifiedMediaAcquisitionDuplicates(run, groups)
    await addMediaAcquisitionEvent(run.id, result.errors.length ? 'warning' : 'info', 'organize', result.errors.length ? `重复集清理完成，${result.removed} 个文件已移入回收站，另有 ${result.errors.length} 个失败。` : `重复集清理完成，${result.removed} 个较小副本已移入回收站。`, {
      tool: 'duplicateEpisodeCleanup',
      removed: result.removed,
      errors: result.errors,
      groups: groups.map(group => ({ episode: group.episode, keep: group.keep.name, removed: group.deleteCandidates.map(file => file.name) }))
    })
    if (result.errors.length) message.warning(`已移入回收站 ${result.removed} 个文件；${result.errors.join('；')}`)
    else message.success(`已将 ${result.removed} 个较小重复集移入回收站`)
    await refresh()
  } catch (error: any) {
    message.error(error?.message || '清理重复集失败')
  } finally {
    cleaningRunId.value = ''
  }
}

onMounted(() => {
  void refresh()
  timer = setInterval(() => void refresh(), 2500)
})

onBeforeUnmount(() => timer && clearInterval(timer))
</script>

<template>
  <section class="media-task-page">
    <header class="media-task-head">
      <div>
        <span class="media-task-kicker">MEDIA ACQUISITION</span>
        <h2>媒体任务</h2>
        <p>任务绑定目标网盘和目录；候选资源由 Agent 或你手动添加，确认后才会转存。</p>
      </div>
      <div class="media-task-head-actions">
        <button type="button" class="media-task-clear" :disabled="clearingCompleted || !completedRuns.length" title="清理已结束任务" @click="clearCompleted"><Trash2 :size="15" />清空</button>
        <button type="button" class="media-task-refresh" :disabled="loading" title="刷新任务" @click="refresh">
          <LoaderCircle v-if="loading" :size="16" class="spin" />
          <RefreshCw v-else :size="16" />
        </button>
      </div>
    </header>

    <div v-if="!runs.length && !loading" class="media-task-empty">
      <Clock3 :size="28" />
      <strong>还没有媒体获取任务</strong>
      <span>在媒体详情中选择目标网盘和目录后，任务会出现在这里。</span>
    </div>

    <div v-else class="media-task-list">
      <section v-if="processingRuns.length" class="media-task-section">
        <h3>获取中 <span>{{ processingRuns.length }}</span></h3>
        <article v-for="run in processingRuns" :key="run.id" class="media-task-card is-active">
          <div class="media-task-card-head">
            <strong>{{ run.target.title }}</strong>
            <span class="media-task-status">{{ statusLabel(run.status) }}</span>
          </div>
          <div class="media-task-meta">{{ run.target.targetPlatform }} · {{ run.target.targetDriveId }} · {{ run.kind === 'season' ? '单季获取' : '媒体获取' }}</div>
          <div class="media-task-progress"><i :style="{ width: `${Math.max(4, run.progress)}%` }" /></div>
          <div class="media-task-activity">{{ run.activity }}</div>
          <div v-if="run.events.length" class="media-task-ticker">
            <div v-for="event in run.events.slice(-8).reverse()" :key="event.id" class="media-task-ticker-row" :class="event.level">
              <span class="media-task-ticker-phase">{{ phaseLabel(event.phase) }}</span>
              <span class="media-task-ticker-message">{{ eventMessage(run, event) }}</span>
              <time>{{ formatTime(event.createdAt) }}</time>
            </div>
          </div>
          <div v-if="run.candidates.length" class="media-task-candidates">
            <div v-for="candidate in run.candidates" :key="candidate.id" :class="['media-task-candidate', candidate.status]">
              <div class="media-task-candidate-main">
                <span>{{ candidate.sourcePlatform }} · {{ candidate.title }}</span>
                <small v-if="candidateHint(run, candidate)">{{ candidateHint(run, candidate) }}</small>
              </div>
              <a-button v-if="candidate.status === 'pending'" size="mini" type="primary" :loading="executingCandidateId === candidate.id" @click="executeCandidate(run, candidate.id)"><Download :size="12" />立即执行</a-button>
              <em v-else>{{ candidateStatusLabel(candidate) }}</em>
            </div>
          </div>
          <div class="media-task-actions">
            <span>{{ formatTime(run.startedAt) }}</span>
            <button v-if="endableStatuses.has(run.status)" type="button" @click="cancel(run)"><X :size="13" />{{ endActionLabel(run.status) }}</button>
          </div>
        </article>
      </section>

      <section v-if="queuedRuns.length" class="media-task-section">
        <h3>排队中 <span>{{ queuedRuns.length }}</span></h3>
        <article v-for="run in queuedRuns" :key="run.id" class="media-task-card">
          <div class="media-task-card-head"><strong>{{ run.target.title }}</strong><span class="media-task-status">{{ statusLabel(run.status) }}</span></div>
          <div class="media-task-meta">{{ run.target.targetPlatform }} · {{ run.target.targetDriveId }} · {{ run.kind === 'season' ? '单季获取' : '媒体获取' }}</div>
          <div class="media-task-activity">{{ run.activity }}</div>
          <div v-if="retryHint(run)" class="media-task-retry">
            <Clock3 :size="13" />
            <span>{{ retryHint(run) }}</span>
            <time v-if="run.nextAttemptAt">下次：{{ formatTime(run.nextAttemptAt) }}</time>
          </div>
          <div v-if="run.status === 'queued'" class="media-task-add-source">
            <a-input v-model.trim="pendingLinks[run.id]" size="small" placeholder="粘贴分享链接、magnet 或 HTTP/HTTPS 链接" />
            <a-input v-model.trim="pendingPasswords[run.id]" size="small" placeholder="提取码" class="media-task-password" />
            <a-button size="small" @click="addShare(run)">添加候选</a-button>
          </div>
          <div class="media-task-actions">
            <span>{{ formatTime(run.startedAt) }}</span>
            <button v-if="endableStatuses.has(run.status)" type="button" @click="cancel(run)"><X :size="13" />{{ endActionLabel(run.status) }}</button>
          </div>
        </article>
      </section>

      <section v-if="completedRuns.length" class="media-task-section">
        <h3>已完成 <span>{{ completedRuns.length }}</span></h3>
        <article v-for="run in completedRuns" :key="run.id" class="media-task-card">
          <div class="media-task-card-head">
            <strong>{{ run.target.title }}</strong>
            <span class="media-task-status" :class="{ error: run.status === 'failed', warning: run.status === 'partial' || run.status === 'no_coverage' }">{{ statusLabel(run.status) }}</span>
          </div>
          <div class="media-task-meta">{{ run.target.targetPlatform }} · {{ formatTime(run.startedAt) }}</div>
          <div v-if="run.errorMessage" class="media-task-error"><CircleAlert :size="14" />{{ run.errorMessage }}</div>
          <div class="media-task-activity">{{ run.status === 'completed' ? '已加入媒体库' : (run.events.at(-1)?.message || run.activity) }}</div>
          <div v-if="duplicateGroups(run).length" class="media-task-duplicate-audit">
            <span>发现 {{ duplicateGroups(run).length }} 组重复集，已按文件大小保留较大版本。</span>
            <a-button size="small" status="warning" :loading="cleaningRunId === run.id" @click="cleanDuplicates(run)"><Trash2 :size="12" />清理较小副本</a-button>
          </div>
          <div v-if="run.events.length" class="media-task-ticker compact">
            <div v-for="event in run.events.slice(-4).reverse()" :key="event.id" class="media-task-ticker-row" :class="event.level">
              <span class="media-task-ticker-phase">{{ phaseLabel(event.phase) }}</span>
              <span class="media-task-ticker-message">{{ event.message }}</span>
              <time>{{ formatTime(event.createdAt) }}</time>
            </div>
          </div>
          <div v-if="sourceRecoverableStatuses.has(run.status)" class="media-task-add-source">
            <a-input v-model.trim="pendingLinks[run.id]" size="small" placeholder="粘贴分享链接、magnet 或 HTTP/HTTPS 链接后由 Agent 执行" />
            <a-input v-model.trim="pendingPasswords[run.id]" size="small" placeholder="提取码" class="media-task-password" />
            <a-button size="small" @click="addShare(run)">交给 Agent</a-button>
          </div>
        </article>
      </section>
    </div>
  </section>
</template>

<style scoped>
.media-task-page { height: 100%; overflow: auto; padding: 32px; color: var(--color-text-1); background: var(--color-bg-1); }
.media-task-head { display: flex; justify-content: space-between; gap: 24px; padding-bottom: 24px; border-bottom: 1px solid var(--color-border-2); }
.media-task-kicker { display: block; color: rgb(var(--primary-6)); font-size: 11px; font-weight: 700; letter-spacing: 1.2px; }
.media-task-head h2 { margin: 5px 0; font-size: 25px; }
.media-task-head p { margin: 0; color: var(--color-text-3); }
.media-task-head-actions { display: flex; align-items: center; gap: 8px; }.media-task-refresh, .media-task-clear { height: 34px; border: 1px solid var(--color-border-2); border-radius: 6px; color: var(--color-text-2); background: transparent; cursor: pointer; }.media-task-refresh { width: 34px; }.media-task-clear { display: inline-flex; align-items: center; gap: 5px; padding: 0 10px; }.media-task-refresh:disabled, .media-task-clear:disabled { cursor: not-allowed; opacity: .45; }
.media-task-empty { min-height: 310px; display: flex; flex-direction: column; gap: 10px; align-items: center; justify-content: center; color: var(--color-text-3); }
.media-task-empty strong { color: var(--color-text-1); }
.media-task-list { display: grid; gap: 28px; padding-top: 24px; max-width: 900px; }
.media-task-section { display: grid; gap: 10px; }
.media-task-section h3 { margin: 0; font-size: 14px; color: var(--color-text-2); }
.media-task-section h3 span { color: rgb(var(--primary-6)); }
.media-task-card { padding: 14px 16px; border: 1px solid var(--color-border-2); border-radius: 7px; background: var(--color-bg-2); }
.media-task-card.is-active { border-color: rgb(var(--primary-6)); }
.media-task-card-head, .media-task-actions { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.media-task-status { color: rgb(var(--primary-6)); font-size: 12px; }.media-task-status.error, .media-task-error { color: rgb(var(--danger-6)); }.media-task-status.warning { color: rgb(var(--warning-6)); }
.media-task-meta, .media-task-activity, .media-task-actions { margin-top: 7px; font-size: 12px; color: var(--color-text-3); }
.media-task-progress { height: 4px; overflow: hidden; margin-top: 12px; border-radius: 99px; background: var(--color-fill-2); }.media-task-progress i { display: block; height: 100%; border-radius: inherit; background: rgb(var(--primary-6)); }
.media-task-actions button { display: inline-flex; gap: 3px; align-items: center; padding: 3px 7px; border: 0; color: var(--color-text-2); background: transparent; cursor: pointer; }.media-task-error { display: flex; gap: 5px; align-items: center; margin-top: 8px; }
.media-task-add-source { display: flex; gap: 7px; margin-top: 12px; }.media-task-password { width: 88px; flex: 0 0 88px; }.media-task-add-source :deep(.arco-input-wrapper) { min-width: 0; }.media-task-add-source :deep(.arco-btn) { flex: 0 0 auto; }
.media-task-retry { display: flex; align-items: center; gap: 6px; margin-top: 9px; padding: 7px 9px; border: 1px solid color-mix(in srgb, rgb(var(--warning-6)) 28%, transparent); border-radius: 6px; color: rgb(var(--warning-6)); background: color-mix(in srgb, rgb(var(--warning-6)) 8%, transparent); font-size: 12px; }.media-task-retry span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.media-task-retry time { margin-left: auto; color: var(--color-text-3); white-space: nowrap; }
.media-task-duplicate-audit { display: flex; align-items: center; justify-content: space-between; gap: 10px; margin-top: 10px; padding: 8px 9px; border: 1px solid color-mix(in srgb, rgb(var(--warning-6)) 24%, transparent); border-radius: 6px; color: rgb(var(--warning-6)); background: color-mix(in srgb, rgb(var(--warning-6)) 7%, transparent); font-size: 12px; }.media-task-duplicate-audit span { min-width: 0; }.media-task-duplicate-audit :deep(.arco-btn) { display: inline-flex; flex: 0 0 auto; gap: 4px; align-items: center; }
.media-task-ticker { display: grid; gap: 4px; margin-top: 10px; padding-left: 9px; border-left: 2px solid var(--color-fill-3); color: var(--color-text-3); font-size: 11px; line-height: 17px; }.media-task-ticker.compact { max-height: 96px; overflow: hidden; }.media-task-ticker-row { display: grid; grid-template-columns: 40px minmax(0, 1fr) auto; gap: 7px; align-items: center; }.media-task-ticker-row.warning .media-task-ticker-phase { color: rgb(var(--warning-6)); }.media-task-ticker-row.error .media-task-ticker-phase { color: rgb(var(--danger-6)); }.media-task-ticker-phase { color: rgb(var(--primary-6)); font-weight: 650; }.media-task-ticker-message { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.media-task-ticker time { color: var(--color-text-4); font-size: 10px; }.media-task-candidates { display: grid; gap: 6px; margin-top: 11px; }.media-task-candidate { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 7px 8px; border-radius: 4px; color: var(--color-text-2); background: var(--color-fill-2); font-size: 12px; }.media-task-candidate.selected { background: color-mix(in srgb, rgb(var(--primary-6)) 12%, var(--color-fill-2)); }.media-task-candidate.failed { background: color-mix(in srgb, rgb(var(--danger-6)) 10%, var(--color-fill-2)); }.media-task-candidate.rejected { opacity: .72; }.media-task-candidate-main { display: grid; min-width: 0; gap: 2px; }.media-task-candidate span, .media-task-candidate small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.media-task-candidate small { color: var(--color-text-3); font-size: 11px; }.media-task-candidate em { flex: 0 0 auto; color: var(--color-text-3); font-style: normal; }.media-task-candidate.failed em { color: rgb(var(--danger-6)); }.media-task-candidate.selected em { color: rgb(var(--primary-6)); }.media-task-candidate :deep(.arco-btn) { display: inline-flex; flex: 0 0 auto; gap: 3px; align-items: center; }
</style>
