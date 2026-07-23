<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { modalCloseAll, modalSelectPanDir } from '../utils/modal'
import { useModalStore, useUserStore } from '../store'
import { isCloud123User, isDrive115User, isGuangyaUser, isPikPakUser } from '../aliapi/utils'
import message from '../utils/message'
import DownDAL from './DownDAL'
import { t } from '../i18n'

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
const urlPlaceholder = computed(() => provider.value === 'cloud123' ? t('transfer.httpLinkPlaceholder') : t('transfer.offlineLinkPlaceholder'))
const providerLabel = computed(() => provider.value === 'cloud123' ? t('drive.cloud123') : provider.value === 'pikpak' ? 'PikPak' : provider.value === 'guangya' ? t('drive.guangya') : provider.value === 'drive115' ? t('drive.drive115') : t('transfer.currentDrive'))
const form = reactive({
  url: '',
  fileName: '',
  dirId: '',
  dirName: t('transfer.defaultOfflineFolder')
})

const handleOpen = () => {
  const preset = modalStore.modalData?.offlineForm
  if (preset) {
    form.url = preset.url || ''
    form.fileName = preset.fileName || ''
    form.dirId = preset.dirId || ''
    form.dirName = preset.dirName || t('transfer.defaultOfflineFolder')
  } else {
    form.url = ''
    form.fileName = ''
    form.dirId = ''
    form.dirName = t('transfer.defaultOfflineFolder')
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
      snapshot.dirName = drive_id === 'drive115' ? t('transfer.rootFolder') : t('transfer.defaultOfflineFolder')
    } else {
      snapshot.dirId = String(selectFile.file_id || '')
      snapshot.dirName = selectFile.name || t('transfer.selected')
    }
    modalStore.showModal('cloud123offline', { offlineForm: snapshot })
  })
}

const handleCreate = async () => {
  if (!provider.value) {
    message.error(t('transfer.offlineUnsupported'))
    return
  }
  const urls = Array.from(new Set(form.url.split(/\r?\n/).map(item => item.trim()).filter(Boolean)))
  if (!urls.length) {
    message.error(t('transfer.enterOfflineUrl'))
    return
  }
  const unsupported = urls.find(url => provider.value === 'cloud123' ? !/^https?:\/\//i.test(url) : provider.value === 'drive115' ? !/^(https?:\/\/|ftp:\/\/|magnet:\?|ed2k:\/\/)/i.test(url) : !/^(https?:\/\/|magnet:\?|ed2k:\/\/)/i.test(url))
  if (unsupported) {
    message.error(provider.value === 'cloud123' ? t('transfer.cloud123HttpOnly') : t('transfer.offlineSupportedLinks'))
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
    else failures.push(`${url.slice(0, 80)}: ${result.message || t('transfer.createFailed')}`)
  }
  okLoading.value = false
  if (!success) {
    message.error(failures[0]?.split(': ')[1] || t('transfer.createOfflineFailed'))
    return
  }
  if (failures.length) message.warning(t('transfer.offlineCreatedPartial', { success, total: urls.length }))
  else message.success(t('transfer.offlineCreatedSuccess', { success, provider: providerLabel.value }))
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
      <span class="modaltitle">{{ t('transfer.createOfflineTask') }}</span>
    </template>
    <div style="width: 520px">
      <a-form ref="formRef" :model="form" layout="vertical">
        <a-form-item field="url" :label="t('transfer.downloadLink')">
          <a-textarea v-model="form.url" :placeholder="t('transfer.onePerLine', { placeholder: urlPlaceholder })" :auto-size="{ minRows: 4, maxRows: 10 }" />
        </a-form-item>
        <a-form-item field="fileName" :label="t('transfer.customFileNameOptional')">
          <a-input v-model.trim="form.fileName" :placeholder="t('transfer.fileNameExample')" />
        </a-form-item>
        <a-form-item field="dirId" :label="t('transfer.saveTo')">
          <a-input-search
            :readonly="true"
            :button-text="t('transfer.chooseFolder')"
            search-button
            :model-value="form.dirName"
            @search="handleSelectDir"
          />
          <div style="margin-top: 6px; color: var(--color-text-3); font-size: 12px">
            {{ t('transfer.offlineProviderTip', { provider: providerLabel }) }}
          </div>
        </a-form-item>
        <div style="display: flex; justify-content: flex-end; gap: 8px">
          <a-button v-if="provider === 'drive115'" type="outline" @click="modalStore.showModal('drive115management', {})">{{ t('transfer.drive115TaskManagement') }}</a-button>
          <a-button type="outline" @click="handleHide">{{ t('common.cancel') }}</a-button>
          <a-button type="primary" :loading="okLoading" @click="handleCreate">{{ t('transfer.create') }}</a-button>
        </div>
      </a-form>
    </div>
  </a-modal>
</template>
