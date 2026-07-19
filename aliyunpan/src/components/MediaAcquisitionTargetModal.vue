<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { MediaAcquisitionRequest } from '@shared/types/mediaAcquisition'
import type { ITokenInfo } from '../user/userstore'
import UserDAL from '../user/userdal'
import { GetDriveID, GetDriveType, isDrive115User, isQuarkUser } from '../aliapi/utils'
import { modalSelectPanDir } from '../utils/modal'
import message from '../utils/message'
import { createMediaAcquisitionRun, createMediaAcquisitionTracking } from '../services/mediaAcquisition/client'
import { runMediaAcquisitionTrackingPatrol, runMediaAcquisitionWorkflow } from '../services/mediaAcquisition/workflowRunner'
import { formatMediaAcquisitionCapability, getMediaAcquisitionCapability, normalizeMediaAcquisitionRootFolder } from '../services/mediaAcquisition/capabilities'
import { getDriveProviderIcon, getDriveProviderLabel } from '../utils/driveProvider'
import useSettingStore from '../setting/settingstore'
import { isPro, requireMediaAcquisitionPro } from '../utils/usageLimit'

const props = defineProps<{ visible: boolean; request: MediaAcquisitionRequest }>()
const emit = defineEmits<{ 'update:visible': [value: boolean]; created: [] }>()

type EligibleAccount = { key: string; token: ITokenInfo; driveId: string; capabilityLabel: string; displayName: string; providerLabel: string; providerIcon: string }
const accounts = ref<EligibleAccount[]>([])
const selectedAccountKey = ref('')
const targetFolderId = ref('')
const targetFolderName = ref('根目录')
const creating = ref(false)
const selectingFolder = ref(false)
const rememberTarget = ref(false)
const trackingScope = ref<'current' | 'all'>('current')
const skipNextFolderReset = ref(false)
const proAllowed = ref(false)
const settingStore = useSettingStore()

const selectedAccount = computed(() => accounts.value.find(account => account.key === selectedAccountKey.value))
const selectedCapability = computed(() => selectedAccount.value ? getMediaAcquisitionCapability(selectedAccount.value.token.tokenfrom) : null)
const isSeries = computed(() => ['tv', 'anime'].includes(props.request.mediaType))

function close() { emit('update:visible', false) }

function resetFolder() {
  const account = selectedAccount.value
  if (!account) return
  const type = GetDriveType(account.token.user_id, account.driveId)
  targetFolderId.value = normalizeMediaAcquisitionRootFolder(account.token.tokenfrom, type.key || account.driveId)
  targetFolderName.value = '根目录'
}

async function loadAccounts() {
  const tokens = await UserDAL.GetUserListFromDB()
  accounts.value = tokens.flatMap(token => {
    const capability = getMediaAcquisitionCapability(token.tokenfrom)
    // 阿里云盘的 default_drive_id 通常是备份盘；Agent 入库固定使用资源盘。
    const driveId = GetDriveID(token.user_id, '') || token.resource_drive_id || token.default_drive_id
    return capability && driveId ? [{
      key: `${token.tokenfrom}:${token.user_id}:${driveId}`,
      token,
      driveId,
      capabilityLabel: formatMediaAcquisitionCapability(capability),
      displayName: token.nick_name || token.user_name || token.name || token.user_id,
      providerLabel: getDriveProviderLabel(token.tokenfrom),
      providerIcon: getDriveProviderIcon(token.tokenfrom)
    }] : []
  })
  rememberTarget.value = settingStore.mediaAcquisitionRememberTarget
  const rememberedAccount = settingStore.mediaAcquisitionRememberTarget
    ? accounts.value.find(account => account.token.user_id === settingStore.mediaAcquisitionTargetUserId && account.driveId === settingStore.mediaAcquisitionTargetDriveId)
    : undefined
  if (rememberedAccount && settingStore.mediaAcquisitionTargetFolderId) {
    skipNextFolderReset.value = true
    selectedAccountKey.value = rememberedAccount.key
    targetFolderId.value = settingStore.mediaAcquisitionTargetFolderId
    targetFolderName.value = settingStore.mediaAcquisitionTargetFolderName || '已选目录'
    return
  }
  selectedAccountKey.value = accounts.value[0]?.key || ''
  resetFolder()
}

async function chooseFolder() {
  const account = selectedAccount.value
  if (!account) return
  selectingFolder.value = true
  try {
    // 115 OpenAPI 的文件浏览直接使用当前 access_token；强制刷新会把仍可用的会话误判为失效。
    const cachedToken = UserDAL.GetUserToken(account.token.user_id)
    let ready: ITokenInfo | null
    if (isDrive115User(account.token) || isQuarkUser(account.token)) {
      ready = cachedToken?.access_token ? cachedToken : account.token
    } else {
      ready = await UserDAL.EnsureUserTokenReady(account.token.user_id, true)
    }
    if (!ready?.access_token) throw new Error('登录状态已失效，请重新登录该网盘')
    Object.assign(account.token, ready)
    account.driveId = GetDriveID(ready.user_id, '') || ready.resource_drive_id || ready.default_drive_id || account.driveId
    modalSelectPanDir('selectdir', targetFolderId.value, (_userId, _driveId, folder) => {
      targetFolderId.value = String(folder?.file_id || targetFolderId.value)
      targetFolderName.value = String(folder?.path || folder?.name || '已选目录')
    }, undefined, undefined, { user_id: ready.user_id, drive_id: account.driveId })
  } catch (error: any) {
    message.error(error?.message || '无法刷新网盘登录状态')
  } finally {
    selectingFolder.value = false
  }
}

async function create() {
  const account = selectedAccount.value
  if (!account || !targetFolderId.value) {
    message.warning('请选择目标网盘和保存目录')
    return
  }
  creating.value = true
  try {
    requireMediaAcquisitionPro()
    await settingStore.updateStore(rememberTarget.value
      ? {
          mediaAcquisitionRememberTarget: true,
          mediaAcquisitionTargetUserId: account.token.user_id,
          mediaAcquisitionTargetDriveId: account.driveId,
          mediaAcquisitionTargetFolderId: targetFolderId.value,
          mediaAcquisitionTargetFolderName: targetFolderName.value
        }
      : {
          mediaAcquisitionRememberTarget: false,
          mediaAcquisitionTargetUserId: '',
          mediaAcquisitionTargetDriveId: '',
          mediaAcquisitionTargetFolderId: '',
          mediaAcquisitionTargetFolderName: ''
        })
    if (props.request.trackingOnly) {
      const allSeasons = [...new Set(props.request.trackingSeasonNumbers || [props.request.seasonNumber])].filter((season): season is number => Number.isInteger(season) && Number(season) > 0)
      const seasonNumbers = trackingScope.value === 'all' ? allSeasons : [props.request.seasonNumber || allSeasons[0]].filter((season): season is number => Number.isInteger(season) && Number(season) > 0)
      if (!props.request.tmdbId || !seasonNumbers.length) throw new Error('缺少追更季信息')
      const subscriptions = []
      for (const seasonNumber of seasonNumbers) {
        subscriptions.push(await createMediaAcquisitionTracking({
          mediaLibraryItemId: props.request.mediaLibraryItemId, tmdbId: props.request.tmdbId, mediaType: props.request.mediaType as 'tv' | 'anime', title: props.request.title, alternativeTitles: props.request.alternativeTitles, year: props.request.year, seasonNumber,
          targetUserId: account.token.user_id, targetDriveId: account.driveId, targetPlatform: account.token.tokenfrom, targetParentFileId: targetFolderId.value,
          preferredQuality: settingStore.mediaAcquisitionPreferredQuality, fetchSubtitles: settingStore.mediaAcquisitionFetchSubtitles,
          preferredLanguage: settingStore.mediaAcquisitionFetchSubtitles ? settingStore.mediaAcquisitionSubtitleLanguage : undefined
        }))
      }
      await runMediaAcquisitionTrackingPatrol({ force: true, trackingIds: subscriptions.map(subscription => subscription.id) })
      message.success(seasonNumbers.length > 1 ? `已追更 ${seasonNumbers.length} 季，Agent 将按已播缺集自动补全` : `已追更第 ${seasonNumbers[0]} 季`)
      emit('created')
      close()
      return
    }
    const missingSeasons = [...new Set(props.request.missingSeasonNumbers || [])].filter((season): season is number => Number.isInteger(season) && season > 0)
    const requestedSeasons = missingSeasons.length ? missingSeasons : [props.request.seasonNumber].filter((season): season is number => typeof season === 'number' && Number.isInteger(season) && season > 0)
    const seasonTargets = isSeries.value ? requestedSeasons.map(seasonNumber => ({ seasonNumber, missingEpisodes: props.request.missingEpisodes?.find(gap => gap.seasonNumber === seasonNumber)?.missingEpisodes || [] })) : []
    const primarySeason = seasonTargets[0]
    const runs = [await createMediaAcquisitionRun({
      kind: missingSeasons.length ? 'missing' : isSeries.value ? 'season' : 'movie', mediaLibraryItemId: props.request.mediaLibraryItemId, tmdbId: props.request.tmdbId,
      mediaType: props.request.mediaType, title: props.request.title, alternativeTitles: props.request.alternativeTitles, year: props.request.year, releaseDate: props.request.releaseDate,
      seasonNumber: primarySeason?.seasonNumber, missingEpisodes: primarySeason?.missingEpisodes, seasonTargets,
      targetUserId: account.token.user_id, targetDriveId: account.driveId, targetPlatform: account.token.tokenfrom,
      targetParentFileId: targetFolderId.value,
      preferredQuality: settingStore.mediaAcquisitionPreferredQuality,
      fetchSubtitles: settingStore.mediaAcquisitionFetchSubtitles,
      preferredLanguage: settingStore.mediaAcquisitionFetchSubtitles ? settingStore.mediaAcquisitionSubtitleLanguage : undefined,
      trackingEnabled: isSeries.value
    })]
    for (const run of runs.filter(run => run.status !== 'reserved')) {
      void runMediaAcquisitionWorkflow(run.id).catch(() => {
        message.warning('Agent 已进入后台队列，将自动继续处理')
      })
    }
    message.success(runs.every(run => run.status === 'reserved') ? '已预定，上映后 Agent 将自动开始获取' : seasonTargets.length > 1 ? `已创建跨 ${seasonTargets.length} 季的补全任务，Agent 正在检索资源` : '媒体任务已创建，Agent 正在检索资源')
    emit('created')
    close()
  } catch (error: any) {
    message.error(error?.message || '创建媒体任务失败')
  } finally {
    creating.value = false
  }
}

watch(() => props.visible, visible => { if (visible) { proAllowed.value = isPro(); trackingScope.value = 'current'; void loadAccounts() } }, { immediate: true })
watch(selectedAccountKey, () => {
  if (skipNextFolderReset.value) {
    skipNextFolderReset.value = false
    return
  }
  resetFolder()
})
</script>

<template>
  <a-modal :visible="visible" :footer="false" :width="560" unmount-on-close :title="request.trackingOnly ? '追更到哪里' : '获取到哪里'" @cancel="close">
    <div class="acquisition-modal">
      <p class="acquisition-subtitle">{{ request.title }}<template v-if="request.missingSeasonNumbers?.length"> · 补全第 {{ request.missingSeasonNumbers.join('、') }} 季</template><template v-else-if="isSeries && request.seasonNumber"> · 第 {{ request.seasonNumber }} 季</template></p>
      <div v-if="!proAllowed" class="acquisition-pro-lock">Agent 获取资源、自动入库和追更巡检仅限 Pro 用户使用。</div>
      <template v-if="request.trackingOnly && (request.trackingSeasonNumbers?.length || 0) > 1">
        <label>追更范围</label>
        <a-radio-group v-model="trackingScope" type="button">
          <a-radio value="current">第 {{ request.seasonNumber }} 季</a-radio>
          <a-radio value="all">全部 {{ request.trackingSeasonNumbers?.length }} 季</a-radio>
        </a-radio-group>
      </template>
      <label>目标网盘</label>
      <a-select v-model="selectedAccountKey" placeholder="选择支持自动获取的网盘">
        <a-option v-for="account in accounts" :key="account.key" :value="account.key" :label="`${account.displayName} · ${account.providerLabel}`">
          <span class="acquisition-account-option">
            <img v-if="account.providerIcon" :src="account.providerIcon" :alt="account.providerLabel" />
            <span v-else class="acquisition-account-icon-fallback">{{ account.providerLabel.slice(0, 1) }}</span>
            <strong class="acquisition-account-copy">{{ account.displayName }} · {{ account.providerLabel }}</strong>
          </span>
        </a-option>
      </a-select>
      <p v-if="selectedCapability" class="acquisition-note">此任务只会使用该网盘的 {{ formatMediaAcquisitionCapability(selectedCapability) }} 能力，不会自动切换其它账号。</p>
      <label>保存目录</label>
      <div class="acquisition-folder"><span>{{ targetFolderName }}</span><a-button type="outline" size="small" :loading="selectingFolder" :disabled="!selectedAccount" @click="chooseFolder">选择目录</a-button></div>
      <a-checkbox v-model="rememberTarget">记住当前网盘和目录，下次直接写入此目录</a-checkbox>
      <div v-if="!accounts.length" class="acquisition-empty">暂无支持分享导入、磁力离线或 HTTP 外链离线的已登录网盘。</div>
      <div class="acquisition-actions"><a-button @click="close">取消</a-button><a-button type="primary" :loading="creating" :disabled="!selectedAccount || !proAllowed" @click="create">{{ proAllowed ? request.trackingOnly ? '开始追更' : '创建获取任务' : '升级 Pro 后使用' }}</a-button></div>
    </div>
  </a-modal>
</template>

<style scoped>
.acquisition-modal { display: grid; gap: 10px; }.acquisition-subtitle { margin: -4px 0 10px; color: var(--color-text-2); font-size: 15px; }.acquisition-modal label { color: var(--color-text-2); font-size: 13px; }.acquisition-note { margin: -2px 0 6px; color: var(--color-text-3); font-size: 12px; }.acquisition-folder { display: flex; align-items: center; justify-content: space-between; min-height: 34px; gap: 12px; padding: 0 8px 0 11px; border: 1px solid var(--color-border-2); border-radius: 4px; color: var(--color-text-2); }.acquisition-empty, .acquisition-pro-lock { padding: 10px; border-radius: 4px; color: rgb(var(--warning-6)); background: var(--color-fill-2); font-size: 13px; }.acquisition-pro-lock { color: rgb(var(--primary-6)); background: rgb(var(--primary-1)); }.acquisition-actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 14px; }.acquisition-account-option { display: flex; align-items: center; gap: 10px; min-width: 0; min-height: 32px; }.acquisition-account-option img, .acquisition-account-icon-fallback { display: inline-flex; flex: 0 0 auto; align-items: center; justify-content: center; width: 26px; height: 26px; border-radius: 6px; object-fit: contain; background: var(--color-fill-2); }.acquisition-account-copy { min-width: 0; overflow: hidden; color: var(--color-text-1); font-size: 14px; font-weight: 500; text-overflow: ellipsis; white-space: nowrap; }:deep(.arco-select-option) { min-height: 44px; padding-top: 6px; padding-bottom: 6px; }
</style>
