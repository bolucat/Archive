<script setup lang="ts">
import { ref } from 'vue'
import { Modal } from '@arco-design/web-vue'
import { modalCloseAll } from '../utils/modal'
import { useModalStore, useUserStore } from '../store'
import message from '../utils/message'
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
  message.success('云下载任务已清理')
  await refresh()
}

const confirmClearTasks = (flag: number) => {
  if (flag !== 1) {
    void clearTasks(flag)
    return
  }
  Modal.confirm({
    title: '清空全部云下载任务？',
    content: '此操作会清除 115 云下载列表中的全部任务，是否继续？',
    onOk: () => clearTasks(flag)
  })
}

const parseTorrent = async () => {
  if (!torrentSha1.value || !torrentPickCode.value) {
    message.error('请填写种子 SHA1 和提取码')
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
  message.success('BT 种子解析完成')
}

const addTorrent = async () => {
  if (!torrentInfoHash.value || !wanted.value.length) {
    message.error('请先解析种子并选择文件')
    return
  }
  if (!savePath.value.trim()) {
    message.error('请填写 BT 保存路径')
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
  message.success('BT 云下载任务已创建')
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
  <a-modal :visible="visible" :footer="false" :unmount-on-close="true" title="115 云下载管理" @before-open="handleOpen" @cancel="modalCloseAll">
    <a-tabs v-model:active-key="activeTab">
      <a-tab-pane key="tasks" title="任务管理">
        <div class="drive115-actions">
          <a-button :loading="loading" @click="refresh">刷新</a-button>
          <a-button @click="confirmClearTasks(0)">清空已完成</a-button>
          <a-button status="danger" @click="confirmClearTasks(1)">清空全部</a-button>
        </div>
        <a-list v-if="tasks.length" :bordered="false">
          <a-list-item v-for="task in tasks" :key="task.info_hash">
            <a-list-item-meta :title="task.name || task.url || task.info_hash" :description="`进度 ${task.percentDone || 0}% · 状态 ${task.status}`" />
          </a-list-item>
        </a-list>
        <a-empty v-else description="暂无云下载任务" />
        <div v-if="quota.length" class="drive115-quota">
          <div v-for="item in quota" :key="`${item.name}-${item.expire_time}`">{{ item.name || '云下载配额' }}：剩余 {{ item.surplus ?? '-' }} / {{ item.count ?? '-' }}</div>
        </div>
      </a-tab-pane>
      <a-tab-pane v-if="hasTorrentPreset" key="torrent" title="BT 任务">
        <a-form layout="vertical">
          <div v-if="torrentFiles.length" class="drive115-torrent-files">
            <div class="drive115-torrent-name">{{ torrentName || 'BT 任务' }}</div>
            <a-checkbox-group v-model="wanted">
              <a-checkbox v-for="file in torrentFiles" :key="file.index" :value="String(file.index)">{{ file.name }}</a-checkbox>
            </a-checkbox-group>
          </div>
          <div v-else class="drive115-torrent-empty">正在解析种子文件...</div>
          <a-form-item label="保存路径"><a-input v-model="savePath" placeholder="例如：Season 01" /></a-form-item>
          <a-button type="primary" :disabled="!torrentFiles.length" @click="addTorrent">创建 BT 任务</a-button>
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
