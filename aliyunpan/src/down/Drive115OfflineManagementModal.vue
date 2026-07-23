<script setup lang="ts">
import { ref } from 'vue'
import { Modal } from '@arco-design/web-vue'
import { modalCloseAll } from '../utils/modal'
import { useModalStore, useUserStore } from '../store'
import message from '../utils/message'
import { t } from '../i18n'
import {
  apiDrive115OfflineAddTorrent,
  apiDrive115OfflineClear,
  apiDrive115OfflineQuota,
  apiDrive115OfflineTasks,
  apiDrive115TorrentParse,
  type Drive115OfflineQuota,
  type Drive115OfflineTask
} from '../cloud115/offline'

defineProps<{ visible: boolean }>()

const userStore = useUserStore()
const modalStore = useModalStore()
const loading = ref(false)
const activeTab = ref('tasks')
const hasTorrentPreset = ref(false)
const tasks = ref<Drive115OfflineTask[]>([])
const quota = ref<Drive115OfflineQuota[]>([])
const torrentSha1 = ref('')
const torrentPickCode = ref('')
const torrentInfoHash = ref('')
const torrentName = ref('')
const torrentFiles = ref<{ index: number; name: string; size: number; wanted: boolean }[]>([])
const wanted = ref<string[]>([])
const savePath = ref('')

const refresh = async () => {
  const userId = userStore.user_id
  if (!userId) return
  loading.value = true
  const [taskResult, quotaResult] = await Promise.all([apiDrive115OfflineTasks(userId), apiDrive115OfflineQuota(userId)])
  loading.value = false
  if (taskResult.error) message.error(taskResult.error)
  if (quotaResult.error) message.error(quotaResult.error)
  tasks.value = taskResult.tasks
  quota.value = quotaResult.items
}

const clearTasks = async (flag: number) => {
  const result = await apiDrive115OfflineClear(userStore.user_id, flag)
  if (!result.success) {
    message.error(result.error)
    return
  }
  message.success(t('transfer.cloudTasksCleared'))
  await refresh()
}

const confirmClearTasks = (flag: number) => {
  if (flag !== 1) {
    void clearTasks(flag)
    return
  }
  Modal.confirm({
    title: t('transfer.clearCloudTasksConfirmTitle'),
    content: t('transfer.clearCloudTasksConfirmContent'),
    onOk: () => clearTasks(flag)
  })
}

const parseTorrent = async () => {
  if (!torrentSha1.value || !torrentPickCode.value) {
    message.error(t('transfer.fillTorrentSha1AndCode'))
    return
  }
  const result = await apiDrive115TorrentParse(userStore.user_id, torrentSha1.value.trim(), torrentPickCode.value.trim())
  if (result.error) {
    message.error(result.error)
    return
  }
  torrentInfoHash.value = result.infoHash
  torrentName.value = result.name
  torrentFiles.value = result.files
  wanted.value = result.files.filter((file: { wanted: boolean }) => file.wanted).map((file: { index: number }) => String(file.index))
  message.success(t('transfer.torrentParsed'))
}

const addTorrent = async () => {
  if (!torrentInfoHash.value || !wanted.value.length) {
    message.error(t('transfer.parseTorrentAndSelectFiles'))
    return
  }
  if (!savePath.value.trim()) {
    message.error(t('transfer.fillBtSavePath'))
    return
  }
  const result = await apiDrive115OfflineAddTorrent(userStore.user_id, {
    infoHash: torrentInfoHash.value,
    wanted: wanted.value.join(','),
    savePath: savePath.value.trim(),
    torrentSha1: torrentSha1.value.trim(),
    pickCode: torrentPickCode.value.trim()
  })
  if (!result.success) {
    message.error(result.error)
    return
  }
  message.success(t('transfer.btCloudTaskCreated'))
  await refresh()
}

const handleOpen = async () => {
  const preset = modalStore.modalData?.torrent
  if (preset?.sha1 && preset?.pickCode) {
    hasTorrentPreset.value = true
    activeTab.value = 'torrent'
    torrentSha1.value = String(preset.sha1)
    torrentPickCode.value = String(preset.pickCode)
    savePath.value = String(preset.name || '')
    torrentInfoHash.value = ''
    torrentName.value = ''
    torrentFiles.value = []
    wanted.value = []
    await parseTorrent()
  } else {
    hasTorrentPreset.value = false
    activeTab.value = 'tasks'
  }
  void refresh()
}
</script>

<template>
  <a-modal :visible="visible" :footer="false" :title="t('transfer.drive115Management')" :unmount-on-close="true" @before-open="handleOpen" @cancel="modalCloseAll">
    <a-tabs v-model:active-key="activeTab">
      <a-tab-pane key="tasks" :title="t('transfer.taskManagement')">
        <div class="drive115-actions">
          <a-button :loading="loading" @click="refresh">{{ t('common.refresh') }}</a-button>
          <a-button @click="confirmClearTasks(0)">{{ t('transfer.clearCompleted') }}</a-button>
          <a-button status="danger" @click="confirmClearTasks(1)">{{ t('transfer.clearAll') }}</a-button>
        </div>
        <a-list v-if="tasks.length" :bordered="false">
          <a-list-item v-for="task in tasks" :key="task.info_hash">
            <a-list-item-meta :title="task.name || task.url || task.info_hash" :description="t('transfer.taskProgressStatus', { progress: task.percentDone || 0, status: task.status || '-' })" />
          </a-list-item>
        </a-list>
        <a-empty v-else :description="t('transfer.noCloudDownloadTasks')" />
        <div v-if="quota.length" class="drive115-quota">
          <div v-for="item in quota" :key="`${item.name}-${item.expire_time}`">{{ t('transfer.quotaRemain', { name: item.name || t('transfer.cloudDownloadQuota'), surplus: item.surplus ?? '-', count: item.count ?? '-' }) }}</div>
        </div>
      </a-tab-pane>
      <a-tab-pane v-if="hasTorrentPreset" key="torrent" :title="t('transfer.btTask')">
        <a-form layout="vertical">
          <div v-if="torrentFiles.length" class="drive115-torrent-files">
            <div class="drive115-torrent-name">{{ torrentName || t('transfer.btTask') }}</div>
            <a-checkbox-group v-model="wanted">
              <a-checkbox v-for="file in torrentFiles" :key="file.index" :value="String(file.index)">{{ file.name }}</a-checkbox>
            </a-checkbox-group>
          </div>
          <div v-else class="drive115-torrent-empty">{{ t('transfer.parsingTorrent') }}</div>
          <a-form-item :label="t('transfer.savePath')"><a-input v-model="savePath" :placeholder="t('transfer.savePathExample')" /></a-form-item>
          <a-button type="primary" :disabled="!torrentFiles.length" @click="addTorrent">{{ t('transfer.createBtTask') }}</a-button>
        </a-form>
      </a-tab-pane>
    </a-tabs>
  </a-modal>
</template>

<style scoped>
.drive115-actions { display: flex; gap: 8px; margin-bottom: 12px; }
.drive115-quota { margin-top: 12px; color: var(--color-text-2); font-size: 12px; }
.drive115-torrent-files { display: grid; gap: 8px; margin: 14px 0; }
.drive115-torrent-files :deep(.arco-checkbox-group) { display: grid; gap: 6px; }
.drive115-torrent-name { font-weight: 600; }
.drive115-torrent-empty { margin: 14px 0; color: var(--color-text-3); }
</style>
