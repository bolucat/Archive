<script setup lang='ts'>
import {
  menuAddAlbumSelectFile,
  menuCopyFileName,
  menuCopyFileTree,
  menuCopySelectedFile,
  menuCreatShare,
  menuDLNA,
  menuDownload,
  menuFileClearHistory,
  menuFileColorChange,
  menuFileEncTypeChange,
  menuDrive115VideoPush,
  menuJumpToDir,
  menuM3U8Download,
  menuTrashSelectFile,
  menuVideoXBT
} from '../topbtns/topbtn'
import { modalRename, modalShuXing } from '../../utils/modal'
import { useSettingStore, usePanFileStore, useAppStore, usePanTreeStore, useModalStore } from '../../store'
import { computed, ref } from 'vue'
import { MediaScanner } from '../../utils/mediaScanner'
import MusicScanner from '../../utils/musicScanner'
import BookScanner from '../../utils/bookScanner'
import message from '../../utils/message'
import { isAliyunUser as isAliyunAccountUser, isBoxUser, isCloud123User, isDrive115User, isDropboxUser, isGuangyaUser, isOneDriveUser, isPikPakUser } from '../../aliapi/utils'
import { isWebDavDrive } from '../../utils/webdavClient'
import { supportsCopy, supportsCreateShare, supportsMove, supportsRename, supportsTrashMove, supportsTrashPermanentDelete } from '../../aliapi/providerFeatures'
import { apiDrive115FileDetailResult } from '../../cloud115/filecmd'
import { t } from '../../i18n'

let istree = false
const settingStore = useSettingStore()
const panFileStore = usePanFileStore()
const appStore = useAppStore()
const panTreeStore = usePanTreeStore()
const modalStore = useModalStore()
const mediaScanner = MediaScanner.getInstance()
const musicScanner = MusicScanner.getInstance()
const bookScanner = BookScanner.getInstance()


const pickFolderForScan = () => {
  const selectedFiles = panFileStore.GetSelected()
  if (selectedFiles.length === 0) {
    message.warning(t('file.selectScanFolder'))
    return null
  }
  const folder = selectedFiles.find((file) => file.isDir)
  if (!folder) {
    message.warning(t('file.scanFolderOnly'))
    return null
  }
  return folder
}

// 扫描类型勾选
const scanVideo = ref(true)
const scanAudio = ref(false)
const scanBook = ref(false)
const isScanning = computed(() => mediaScanner.isCurrentlyScanning || musicScanner.isScanning || bookScanner.isScanning)

const handleStartScan = async () => {
  const folder = pickFolderForScan()
  if (!folder) return
  if (isScanning.value) { message.warning(t('file.scanning')); return }
  if (!scanVideo.value && !scanAudio.value && !scanBook.value) { message.warning(t('file.selectScanType')); return }

  const userId = (folder as any).user_id || panTreeStore.user_id || ''
  const tasks: Promise<any>[] = []

  if (scanVideo.value && !mediaScanner.isCurrentlyScanning) {
    message.info(`开始扫描 "${folder.name}" 视频`)
    appStore.toggleTab('media')
    tasks.push(mediaScanner.scanFolder(folder, folder.drive_id).catch(e => console.error('视频扫描失败:', e)))
  }
  if (scanAudio.value && !musicScanner.isScanning) {
    if (!userId) { message.error(t('file.accountMissing')); return }
    appStore.toggleTab('music')
    tasks.push(musicScanner.scanFolder(folder, userId).then(r => message.success(`音频扫描完成: ${r.found} 首`)).catch(e => console.error('音频扫描失败:', e)))
  }
  if (scanBook.value && !bookScanner.isScanning) {
    if (!userId) { message.error(t('file.accountMissing')); return }
    appStore.toggleTab('book')
    tasks.push(bookScanner.scanFolder(folder, userId).then(r => message.success(`书籍扫描完成: ${r.found} 本`)).catch(e => console.error('书籍扫描失败:', e)))
  }

  await Promise.allSettled(tasks)
}

// AI 批量刮削
const handleAIBatchScrape = async () => {
  const folder = pickFolderForScan()
  if (!folder) return
  if (mediaScanner.isCurrentlyScanning) {
    message.warning(t('file.scanning'))
    return
  }
  try {
    appStore.toggleTab('media')
    await mediaScanner.batchAIScrapeFolder(folder, (folder as any).drive_id)
  } catch (error) {
    console.error('AI 批量刮削失败:', error)
    message.error(t('file.aiRescrapeFailed'))
  }
}

const props = defineProps({
  dirtype: {
    type: String,
    required: true
  },
  isvideo: {
    type: Boolean,
    required: true
  },
  isselected: {
    type: Boolean,
    required: true
  },
  isselectedmulti: {
    type: Boolean,
    required: true
  },
  isallfavored: {
    type: Boolean,
    required: true
  },
  inputselectType: {
    type: String,
    required: true
  },
  inputpicType: {
    type: String,
    required: true
  }
})

const isShowBtn = computed(() => {
  return (props.dirtype === 'pic' && props.inputpicType != 'mypic')
    || props.dirtype === 'mypic' || props.dirtype === 'pan'
})
const isPic = computed(() => {
  return (props.dirtype === 'pic' && props.inputpicType == 'mypic')
})
const isCloudUser = computed(() => isCloud123User(panTreeStore.user_id || '') || panTreeStore.drive_id === 'cloud123')
const isAliyunAccount = computed(() => isAliyunAccountUser(panTreeStore.user_id || ''))
const isDropbox = computed(() => isDropboxUser(panTreeStore.user_id || '') || panTreeStore.drive_id === 'dropbox')
const isOneDrive = computed(() => isOneDriveUser(panTreeStore.user_id || '') || panTreeStore.drive_id === 'onedrive')
const isBox = computed(() => isBoxUser(panTreeStore.user_id || '') || panTreeStore.drive_id === 'box')
const isGuangya = computed(() => isGuangyaUser(panTreeStore.user_id || '') || panTreeStore.drive_id === 'guangya')
const isPikPak = computed(() => isPikPakUser(panTreeStore.user_id || '') || panTreeStore.drive_id === 'pikpak')
const isThirdPartyDrive = computed(() => isDropbox.value || isOneDrive.value || isBox.value || isGuangya.value || isPikPak.value)
const isShareSupported = computed(() => supportsCreateShare(panTreeStore.user_id || '', panTreeStore.drive_id || ''))
const isCopySupported = computed(() => supportsCopy(panTreeStore.user_id || '', panTreeStore.drive_id || ''))
const isMoveSupported = computed(() => supportsMove(panTreeStore.user_id || '', panTreeStore.drive_id || ''))
const isRenameSupported = computed(() => supportsRename(panTreeStore.user_id || '', panTreeStore.drive_id || ''))
const isTrashSupported = computed(() => supportsTrashMove(panTreeStore.user_id || '', panTreeStore.drive_id || ''))
const isPermanentDeleteSupported = computed(() => supportsTrashPermanentDelete(panTreeStore.user_id || '', panTreeStore.drive_id || ''))
const hasFileOperations = computed(() => isMoveSupported.value || isCopySupported.value || isTrashSupported.value || isPermanentDeleteSupported.value || props.dirtype === 'mypic')
const isWebDav = computed(() => isWebDavDrive(panTreeStore.drive_id || panTreeStore.selectDir.drive_id))
const isDrive115Video = computed(() => {
  const selected = panFileStore.GetSelected()
  const file = selected[0]
  return props.isvideo && selected.length === 1 && !!file && !file.isDir && (isDrive115User((file as any).user_id || panTreeStore.user_id || '') || file.drive_id === 'drive115')
})
const isDrive115Torrent = computed(() => {
  const selected = panFileStore.GetSelected()
  const file = selected[0]
  return selected.length === 1 && !!file && !file.isDir && String(file.name || '').toLowerCase().endsWith('.torrent')
    && (isDrive115User((file as any).user_id || panTreeStore.user_id || '') || file.drive_id === 'drive115')
})

const openDrive115Torrent = async () => {
  const file = panFileStore.GetSelected()[0]
  if (!file || !isDrive115Torrent.value) return
  const userId = (file as any).user_id || panTreeStore.user_id || ''
  const result = await apiDrive115FileDetailResult(userId, file.file_id)
  if (!result.detail?.sha1 || !result.detail?.pick_code) {
    message.error(result.error || '无法读取 115 种子文件信息')
    return
  }
  modalStore.showModal('drive115management', {
    torrent: {
      sha1: result.detail.sha1,
      pickCode: result.detail.pick_code,
      name: String(file.name || '').replace(/\.torrent$/i, '')
    }
  })
}

// 检查是否选中了文件夹
const isSelectedFolder = computed(() => {
  const selectedFiles = panFileStore.GetSelected()
  return selectedFiles.some(file => file.isDir)
})

const isDocumentAIAvailable = computed(() => {
  const selected = panFileStore.GetSelected()
  if (selected.length !== 1 || selected[0].isDir) return false
  const name = String(selected[0].name || '').toLowerCase()
  return ['.pdf', '.docx', '.epub', '.txt', '.md', '.markdown'].some(extension => name.endsWith(extension))
})

function openDocumentAI() {
  const selected = panFileStore.GetSelected()
  if (selected.length !== 1 || !isDocumentAIAvailable.value) {
    message.warning(t('file.documentAiUnsupported'))
    return
  }
  const file = selected[0]
  sessionStorage.setItem('boxplayer:pending-document-ai', JSON.stringify({ file, userId: (file as any).user_id || panTreeStore.user_id || '' }))
  window.dispatchEvent(new CustomEvent('boxplayer:open-document-ai'))
  appStore.toggleTab('ai-workspace')
}
</script>

<template>
  <a-dropdown id='rightpanmenu' class='rightmenu' :popup-visible='true' style='z-index: -1; left: -200px; opacity: 0'>
    <template #content>
      <a-doption @click='() => menuDownload(istree)'>
        <template #icon><IconFont name="icondownload" /></template>
        <template #default>{{ t('file.download') }}</template>
      </a-doption>
      <a-doption v-if='isDocumentAIAvailable' @click='openDocumentAI'>
        <template #icon><IconFont name="iconscan" /></template>
        <template #default>{{ t('file.analyzeWithAi') }} <span class="ai-pro-badge">Pro</span></template>
      </a-doption>
      <a-doption v-if='isDrive115Torrent' @click='openDrive115Torrent'>
        <template #icon><IconFont name="icondownload" /></template>
        <template #default>{{ t('file.drive115CloudDownload') }}</template>
      </a-doption>
      <a-doption v-show='isShareSupported'
                 @click="() => menuCreatShare(istree, 'pan', 'resource_root')">
        <template #icon><IconFont name="iconfenxiang" /></template>
        <template #default>{{ t('file.share') }}</template>
      </a-doption>
      <a-doption v-if="isAliyunAccount" @click="() => menuCreatShare(istree, 'pan', 'backup_root')">
        <template #icon><IconFont name="iconrss" /></template>
        <template #default>{{ t('file.quickTransfer') }}</template>
      </a-doption>

      <!-- 扫描数据 -->
      <a-dsubmenu v-if="isSelectedFolder && isShowBtn" class='rightmenu' trigger='hover'>
        <template #default>
          <div @click.stop='() => {}'>
            <span class='arco-dropdown-option-icon'>
              <IconFont name="iconscan" style='opacity: 0.8' />
            </span>
            {{ t('file.scan') }}
          </div>
        </template>
        <template #content>
          <a-doption @click.stop="scanVideo = !scanVideo">
            <template #icon>
              <IconFont :name="scanVideo ? 'iconcheckbox-full' : 'iconfangkuang'" :style="scanVideo ? 'color: rgb(var(--primary-6))' : ''" />
            </template>
            <template #default>{{ t('file.video') }}</template>
          </a-doption>
          <a-doption @click.stop="scanAudio = !scanAudio">
            <template #icon>
              <IconFont :name="scanAudio ? 'iconcheckbox-full' : 'iconfangkuang'" :style="scanAudio ? 'color: rgb(var(--primary-6))' : ''" />
            </template>
            <template #default>{{ t('file.audio') }}</template>
          </a-doption>
          <a-doption @click.stop="scanBook = !scanBook">
            <template #icon>
              <IconFont :name="scanBook ? 'iconcheckbox-full' : 'iconfangkuang'" :style="scanBook ? 'color: rgb(var(--primary-6))' : ''" />
            </template>
            <template #default>{{ t('file.book') }}</template>
          </a-doption>
          <a-doption @click="handleStartScan">
            <template #icon><IconFont name="iconstart" /></template>
            <template #default>{{ t('file.startScan') }}</template>
          </a-doption>
          <a-doption @click="handleAIBatchScrape">
            <template #icon><IconFont name="iconscan" /></template>
            <template #default>{{ t('file.aiRescrape') }} <span class="ai-pro-badge">Pro</span></template>
          </a-doption>
        </template>
      </a-dsubmenu>

      <a-dsubmenu v-if="dirtype !== 'pic' && !isWebDav && isAliyunAccount" id='rightpansubbiaoji' class='rightmenu' trigger='hover'>
        <template #default>
          <div @click.stop='() => {}'>
            <span class='arco-dropdown-option-icon'>
              <IconFont name="iconwbiaoqian" style='opacity: 0.8' />
            </span>{{ t('file.mark') }}
          </div>
        </template>
        <template #content>
          <a-doption v-for='item in settingStore.uiFileColorArray' :key='item.key'
                     @click='() => menuFileColorChange(istree, item.key)'>
            <template #icon><IconFont name="iconcheckbox-full" :style='{ color: item.key }' /></template>
            <template #default>{{ item.title || item.key }}</template>
          </a-doption>

          <a-doption @click="() => menuFileColorChange(istree, '#e74c3c')">
            <template #icon><IconFont name="iconcheckbox-full" style='color: #e74c3c' /></template>
            <template #default>{{ t('file.videoRed') }}</template>
          </a-doption>
          <a-doption @click="() => menuFileColorChange(istree, '')">
            <template #icon><IconFont name="iconfangkuang" /></template>
            <template #default>{{ t('file.clearMark') }}</template>
          </a-doption>
        </template>
      </a-dsubmenu>
      <a-dsubmenu v-if="dirtype != 'video' && hasFileOperations" id='rightpansubmove' class='rightmenu' trigger='hover'>
        <template #default>
          <div @click.stop='() => {}'>
            <span class='arco-dropdown-option-icon'>
              <IconFont name="iconmoveto" style='opacity: 0.8' />
            </span>
            {{ t('file.operations') }}
          </div>
        </template>
        <template #content>
          <a-doption v-show='isShowBtn && inputpicType !== "mypic" && dirtype !== "pan"'
                     @click='() => menuAddAlbumSelectFile()'>
            <template #icon><IconFont name="iconmoveto" /></template>
            <template #default>{{ t('file.moveToAlbum') }}</template>
          </a-doption>
          <a-doption v-show='dirtype === "mypic"'
                     @click='() => menuTrashSelectFile(istree, false, true)'>
            <template #icon><IconFont name="iconqingkong" /></template>
            <template #default>{{ t('file.removeFromAlbum') }}</template>
          </a-doption>
          <a-doption v-show='isShowBtn && isMoveSupported' @click="() => menuCopySelectedFile(istree, 'cut')">
            <template #icon><IconFont name="iconscissor" /></template>
            <template #default>{{ t('file.moveTo') }}</template>
          </a-doption>
          <a-doption v-show='isShowBtn && isCopySupported' @click="() => menuCopySelectedFile(istree, 'copy')">
            <template #icon><IconFont name="iconcopy" /></template>
            <template #default>{{ t('file.copyTo') }}</template>
          </a-doption>
          <a-doption v-show='isShowBtn && isAliyunAccount && !isWebDav && !isThirdPartyDrive' type='text' size='small' tabindex='-1' title='Ctrl+M'
                     @click="() => menuFileEncTypeChange(istree)">
            <template #icon><IconFont name="iconsafebox" /></template>
            <template #default>{{ t('file.markEncrypted') }}</template>
          </a-doption>
          <a-doption v-show='isTrashSupported && (isShowBtn && dirtype !== "mypic"  || dirtype === "search")' class='danger' @click='() => menuTrashSelectFile(istree, false, isPic)'>
            <template #icon><IconFont name="icondelete" /></template>
            <template #default>{{ t('file.trash') }}</template>
          </a-doption>
          <a-dsubmenu v-if='dirtype !== "mypic" && isPermanentDeleteSupported' class='rightmenu' trigger='hover'>
            <template #default>
              <span class='arco-dropdown-option-icon'><IconFont name="iconrest" /></span>{{ t('file.deletePermanently') }}
            </template>
            <template #content>
              <a-doption title='Ctrl+Shift+Delete' class='danger' @click='() => menuTrashSelectFile(istree, true, isPic)'>
                <template #default>{{ t('file.cannotRestore') }}</template>
              </a-doption>
            </template>
          </a-dsubmenu>
        </template>
      </a-dsubmenu>

      <a-doption v-show="dirtype != 'video' && isRenameSupported"
                 @click='() => modalRename(istree, isselectedmulti, dirtype.includes("pic"))'>
        <template #icon><IconFont name="iconedit-square" /></template>
        <template #default>{{ t('file.rename') }}</template>
      </a-doption>

      <a-doption v-show="!isPic" @click='() => modalShuXing(istree, dirtype.includes("pic"))'>
        <template #icon><IconFont name="iconshuxing" /></template>
        <template #default>{{ t('file.properties') }}</template>
      </a-doption>
      <a-dsubmenu v-if='!dirtype.includes("pic")'
                  id='rightpansubmore' class='rightmenu' trigger='hover'>
        <template #default>
          <div @click.stop='() => {}'>
            <span class='arco-dropdown-option-icon'>
              <IconFont name="icongengduo1" style='opacity: 0.8' />
            </span>
            {{ t('file.more') }}
          </div>
        </template>
        <template #content>
          <a-doption
            v-show="isselected && !isselectedmulti && (dirtype == 'favorite' || dirtype == 'search' || dirtype == 'color' || dirtype == 'video')"
            @click='() => menuJumpToDir()'>
            <template #icon><IconFont name="icondakaiwenjianjia1" /></template>
            <template #default>{{ t('file.openLocation') }}</template>
          </a-doption>
          <a-doption v-show='isvideo' @click='() => menuVideoXBT()'>
            <template #icon><IconFont name="iconjietu" /></template>
            <template #default>{{ t('file.sprite') }}</template>
          </a-doption>
          <a-doption v-show='isShowBtn && isAliyunAccount && !isWebDav && !isThirdPartyDrive' type='text' size='small' tabindex='-1' title='Ctrl+M'
                     @click="() => menuFileEncTypeChange(istree)">
            <template #icon><IconFont name="iconsafebox" /></template>
            <template #default>{{ t('file.markEncrypted') }}</template>
          </a-doption>
          <a-doption v-show='isShowBtn && isAliyunAccount && !isWebDav' type='text' size='small' tabindex='-1' title='Ctrl+M'
                     @click="() => menuFileClearHistory(istree)">
            <template #icon><IconFont name="iconshipin" /></template>
            <template #default>{{ t('file.clearHistory') }}</template>
          </a-doption>
          <a-doption v-show="isvideo" @click="() => menuDLNA()">
            <template #icon><IconFont name="icontouping2" /></template>
            <template #default>{{ t('file.dlna') }}</template>
          </a-doption>
          <a-doption v-show='isvideo' @click='() => menuM3U8Download()'>
            <template #icon><IconFont name="iconluxiang" /></template>
            <template #default>{{ t('file.m3u8Download') }}</template>
          </a-doption>
          <a-dsubmenu v-if='isDrive115Video' class='rightmenu' trigger='hover'>
            <template #default>
              <span class='arco-dropdown-option-icon'><IconFont name="iconshipin" /></span>{{ t('file.drive115Transcode') }}
            </template>
            <template #content>
              <a-doption @click='() => menuDrive115VideoPush("vip_push")'>{{ t('file.vipTranscode') }}</a-doption>
              <a-doption @click='() => menuDrive115VideoPush("pay_push")'>{{ t('file.mapleTranscode') }}</a-doption>
            </template>
          </a-dsubmenu>
          <a-doption v-show='isselected' @click='() => menuCopyFileName()'>
            <template #icon><IconFont name="iconlist" /></template>
            <template #default>{{ t('file.copyName') }}</template>
          </a-doption>
          <a-doption v-show='isselected && !isselectedmulti && !isCloudUser && !isThirdPartyDrive'
                     @click='() => menuCopyFileTree()'>
            <template #icon><IconFont name="iconnode-tree1" /></template>
            <template #default>{{ t('file.copyTree') }}</template>
          </a-doption>
        </template>
      </a-dsubmenu>
    </template>
  </a-dropdown>
</template>
<style>
.ai-pro-badge { display: inline-flex; align-items: center; justify-content: center; border-radius: 999px; background: linear-gradient(135deg, #f59e0b, #f97316); color: #fff; font-weight: 700; line-height: 1; height: 14px; padding: 0 5px; font-size: 9px; vertical-align: middle; margin-left: 4px; }
</style>
