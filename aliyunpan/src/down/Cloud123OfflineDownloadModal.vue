<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { modalCloseAll, modalSelectPanDir } from '../utils/modal'
import { useModalStore, useUserStore } from '../store'
import { isCloud123User, isDrive115User, isGuangyaUser, isPikPakUser } from '../aliapi/utils'
import message from '../utils/message'
import DownDAL from './DownDAL'

const props = defineProps({
  visible: {
    type: Boolean,
    required: true
  }
})

const formRef = ref()
const okLoading = ref(false)
const modalStore = useModalStore()
const userStore = useUserStore()
const provider = computed(() => {
  const user = userStore.user_id || ''
  if (isCloud123User(user)) return 'cloud123'
  if (isPikPakUser(user)) return 'pikpak'
  if (isGuangyaUser(user)) return 'guangya'
  if (isDrive115User(user)) return 'drive115'
  return ''
})
const urlPlaceholder = computed(() => provider.value === 'cloud123' ? 'http/https 链接' : 'http/https、FTP、magnet 或 ed2k 链接')
const providerLabel = computed(() => provider.value === 'cloud123' ? '123 网盘' : provider.value === 'pikpak' ? 'PikPak' : provider.value === 'guangya' ? '光鸭云盘' : provider.value === 'drive115' ? '115 网盘' : '当前网盘')
const form = reactive({
  url: '',
  fileName: '',
  dirId: '',
  dirName: '默认（来自:离线下载）'
})

const handleOpen = () => {
  const preset = modalStore.modalData?.offlineForm
  if (preset) {
    form.url = preset.url || ''
    form.fileName = preset.fileName || ''
    form.dirId = preset.dirId || ''
    form.dirName = preset.dirName || '默认（来自:离线下载）'
  } else {
    form.url = ''
    form.fileName = ''
    form.dirId = ''
    form.dirName = '默认（来自:离线下载）'
  }
}

const handleClose = () => {
  if (okLoading.value) okLoading.value = false
}

const handleHide = () => {
  modalCloseAll()
}

const handleSelectDir = () => {
  const snapshot = {
    url: form.url,
    fileName: form.fileName,
    dirId: form.dirId,
    dirName: form.dirName
  }
  modalSelectPanDir('offline', form.dirId, (user_id: string, drive_id: string, selectFile: any) => {
    if (!selectFile || selectFile.isDir !== true) return
    if (selectFile.file_id && String(selectFile.file_id).includes('root')) {
      snapshot.dirId = drive_id === 'drive115' ? '0' : ''
      snapshot.dirName = drive_id === 'drive115' ? '根目录' : '默认（来自:离线下载）'
    } else {
      snapshot.dirId = String(selectFile.file_id || '')
      snapshot.dirName = selectFile.name || '已选择'
    }
    modalStore.showModal('cloud123offline', { offlineForm: snapshot })
  })
}

const handleCreate = async () => {
  if (!provider.value) {
    message.error('当前账号不支持离线下载')
    return
  }
  const urls = Array.from(new Set(form.url.split(/\r?\n/).map(item => item.trim()).filter(Boolean)))
  if (!urls.length) {
    message.error('请输入离线下载地址')
    return
  }
  const unsupported = urls.find(url => provider.value === 'cloud123' ? !/^https?:\/\//i.test(url) : provider.value === 'drive115' ? !/^(https?:\/\/|ftp:\/\/|magnet:\?|ed2k:\/\/)/i.test(url) : !/^(https?:\/\/|magnet:\?|ed2k:\/\/)/i.test(url))
  if (unsupported) {
    message.error(provider.value === 'cloud123' ? '123 网盘仅支持 http/https 链接' : '仅支持 http/https、FTP、magnet 或 ed2k 链接')
    return
  }
  okLoading.value = true
  let success = 0
  const failures: string[] = []
  for (const url of urls) {
    const result = provider.value === 'drive115'
      ? await DownDAL.aAddDrive115OfflineDownload(url, form.dirId)
      : provider.value === 'pikpak'
      ? await DownDAL.aAddPikPakOfflineDownload(url, urls.length === 1 ? form.fileName.trim() : '', form.dirId)
      : provider.value === 'guangya'
        ? await DownDAL.aAddGuangyaOfflineDownload(url, urls.length === 1 ? form.fileName.trim() : '', form.dirId)
        : await DownDAL.aAddCloud123OfflineDownload(url, urls.length === 1 ? form.fileName.trim() : '', form.dirId)
    if (result.success) success++
    else failures.push(`${url.slice(0, 80)}：${result.message || '创建失败'}`)
  }
  okLoading.value = false
  if (!success) {
    message.error(failures[0]?.split('：')[1] || '创建离线下载失败')
    return
  }
  if (failures.length) message.warning(`已创建 ${success}/${urls.length} 个离线任务`)
  else message.success(`已创建 ${success} 个 ${providerLabel.value} 离线任务`)
  modalCloseAll()
}
</script>

<template>
  <a-modal
    :visible="props.visible"
    modal-class="modalclass"
    :footer="false"
    :unmount-on-close="true"
    :mask-closable="false"
    @cancel="handleHide"
    @before-open="handleOpen"
    @close="handleClose"
  >
    <template #title>
      <span class="modaltitle">创建离线下载任务</span>
    </template>
    <div style="width: 520px">
      <a-form ref="formRef" :model="form" layout="vertical">
        <a-form-item field="url" label="下载链接">
          <a-textarea v-model="form.url" :placeholder="`${urlPlaceholder}，每行一个`" :auto-size="{ minRows: 4, maxRows: 10 }" />
        </a-form-item>
        <a-form-item field="fileName" label="自定义文件名（可选）">
          <a-input v-model.trim="form.fileName" placeholder="例如：视频.mp4" />
        </a-form-item>
        <a-form-item field="dirId" label="保存到">
          <a-input-search
            :readonly="true"
            button-text="选择文件夹"
            search-button
            :model-value="form.dirName"
            @search="handleSelectDir"
          />
          <div style="margin-top: 6px; color: var(--color-text-3); font-size: 12px">
            当前 provider：{{ providerLabel }}。115 网盘可选择根目录；其他网盘未选择时将保存到默认离线下载目录。
          </div>
        </a-form-item>
        <div style="display: flex; justify-content: flex-end; gap: 8px">
          <a-button v-if="provider === 'drive115'" type="outline" @click="modalStore.showModal('drive115management', {})">115 任务管理</a-button>
          <a-button type="outline" @click="handleHide">取消</a-button>
          <a-button type="primary" :loading="okLoading" @click="handleCreate">创建</a-button>
        </div>
      </a-form>
    </div>
  </a-modal>
</template>
