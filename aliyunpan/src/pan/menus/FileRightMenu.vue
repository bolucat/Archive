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
import { supportsCopy, supportsCreateShare } from '../../aliapi/providerFeatures'
import { apiDrive115FileDetailResult } from '../../cloud115/filecmd'

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
    message.warning('请先选择要扫描的文件夹')
    return null
  }
  const folder = selectedFiles.find((file) => file.isDir)
  if (!folder) {
    message.warning('请选择文件夹进行扫描')
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
  if (isScanning.value) { message.warning('正在扫描中，请稍后...'); return }
  if (!scanVideo.value && !scanAudio.value && !scanBook.value) { message.warning('请至少勾选一种扫描类型'); return }

  const userId = (folder as any).user_id || panTreeStore.user_id || ''
  const tasks: Promise<any>[] = []

  if (scanVideo.value && !mediaScanner.isCurrentlyScanning) {
    message.info(`开始扫描 "${folder.name}" 视频`)
    appStore.toggleTab('media')
    tasks.push(mediaScanner.scanFolder(folder, folder.drive_id).catch(e => console.error('视频扫描失败:', e)))
  }
  if (scanAudio.value && !musicScanner.isScanning) {
    if (!userId) { message.error('无法识别当前账号'); return }
    appStore.toggleTab('music')
    tasks.push(musicScanner.scanFolder(folder, userId).then(r => message.success(`音频扫描完成: ${r.found} 首`)).catch(e => console.error('音频扫描失败:', e)))
  }
  if (scanBook.value && !bookScanner.isScanning) {
    if (!userId) { message.error('无法识别当前账号'); return }
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
    message.warning('正在扫描中，请稍后...')
    return
  }
  try {
    appStore.toggleTab('media')
    await mediaScanner.batchAIScrapeFolder(folder, (folder as any).drive_id)
  } catch (error) {
    console.error('AI 批量刮削失败:', error)
    message.error('AI 批量刮削失败，请稍后重试')
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
    message.warning('请选择一个 PDF、DOCX、EPUB、TXT 或 Markdown 文档')
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
        <template #default>下载</template>
      </a-doption>
      <a-doption v-if='isDocumentAIAvailable' @click='openDocumentAI'>
        <template #icon><IconFont name="iconscan" /></template>
        <template #default>用 AI 分析 <span class="ai-pro-badge">Pro</span></template>
      </a-doption>
      <a-doption v-if='isDrive115Torrent' @click='openDrive115Torrent'>
        <template #icon><IconFont name="icondownload" /></template>
        <template #default>115 云下载</template>
      </a-doption>
      <a-doption v-show='isShareSupported'
                 @click="() => menuCreatShare(istree, 'pan', 'resource_root')">
        <template #icon><IconFont name="iconfenxiang" /></template>
        <template #default>分享</template>
      </a-doption>
      <a-doption v-if="isAliyunAccount" @click="() => menuCreatShare(istree, 'pan', 'backup_root')">
        <template #icon><IconFont name="iconrss" /></template>
        <template #default>快传</template>
      </a-doption>

      <!-- 扫描数据 -->
      <a-dsubmenu v-if="isSelectedFolder && isShowBtn" class='rightmenu' trigger='hover'>
        <template #default>
          <div @click.stop='() => {}'>
            <span class='arco-dropdown-option-icon'>
              <IconFont name="iconscan" style='opacity: 0.8' />
            </span>
            扫描数据
          </div>
        </template>
        <template #content>
          <a-doption @click.stop="scanVideo = !scanVideo">
            <template #icon>
              <IconFont :name="scanVideo ? 'iconcheckbox-full' : 'iconfangkuang'" :style="scanVideo ? 'color: rgb(var(--primary-6))' : ''" />
            </template>
            <template #default>视频</template>
          </a-doption>
          <a-doption @click.stop="scanAudio = !scanAudio">
            <template #icon>
              <IconFont :name="scanAudio ? 'iconcheckbox-full' : 'iconfangkuang'" :style="scanAudio ? 'color: rgb(var(--primary-6))' : ''" />
            </template>
            <template #default>音频</template>
          </a-doption>
          <a-doption @click.stop="scanBook = !scanBook">
            <template #icon>
              <IconFont :name="scanBook ? 'iconcheckbox-full' : 'iconfangkuang'" :style="scanBook ? 'color: rgb(var(--primary-6))' : ''" />
            </template>
            <template #default>书籍</template>
          </a-doption>
          <a-doption @click="handleStartScan">
            <template #icon><IconFont name="iconstart" /></template>
            <template #default>开始扫描</template>
          </a-doption>
          <a-doption @click="handleAIBatchScrape">
            <template #icon><IconFont name="iconscan" /></template>
            <template #default>AI 重刮削 <span class="ai-pro-badge">Pro</span></template>
          </a-doption>
        </template>
      </a-dsubmenu>

      <a-dsubmenu v-if="dirtype !== 'pic' && !isWebDav && isAliyunAccount" id='rightpansubbiaoji' class='rightmenu' trigger='hover'>
        <template #default>
          <div @click.stop='() => {}'>
            <span class='arco-dropdown-option-icon'>
              <IconFont name="iconwbiaoqian" style='opacity: 0.8' />
            </span>标记
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
            <template #default>视频红</template>
          </a-doption>
          <a-doption @click="() => menuFileColorChange(istree, '')">
            <template #icon><IconFont name="iconfangkuang" /></template>
            <template #default>清除标记</template>
          </a-doption>
        </template>
      </a-dsubmenu>
      <a-dsubmenu v-if="dirtype != 'video'" id='rightpansubmove' class='rightmenu' trigger='hover'>
        <template #default>
          <div @click.stop='() => {}'>
            <span class='arco-dropdown-option-icon'>
              <IconFont name="iconmoveto" style='opacity: 0.8' />
            </span>
            操作
          </div>
        </template>
        <template #content>
          <a-doption v-show='isShowBtn && inputpicType !== "mypic" && dirtype !== "pan"'
                     @click='() => menuAddAlbumSelectFile()'>
            <template #icon><IconFont name="iconmoveto" /></template>
            <template #default>移入相册</template>
          </a-doption>
          <a-doption v-show='dirtype === "mypic"'
                     @click='() => menuTrashSelectFile(istree, false, true)'>
            <template #icon><IconFont name="iconqingkong" /></template>
            <template #default>移出相册</template>
          </a-doption>
          <a-doption v-show='isShowBtn' @click="() => menuCopySelectedFile(istree, 'cut')">
            <template #icon><IconFont name="iconscissor" /></template>
            <template #default>移动到...</template>
          </a-doption>
          <a-doption v-show='isShowBtn && isCopySupported' @click="() => menuCopySelectedFile(istree, 'copy')">
            <template #icon><IconFont name="iconcopy" /></template>
            <template #default>复制到...</template>
          </a-doption>
          <a-doption v-show='isShowBtn && isAliyunAccount && !isWebDav && !isThirdPartyDrive' type='text' size='small' tabindex='-1' title='Ctrl+M'
                     @click="() => menuFileEncTypeChange(istree)">
            <template #icon><IconFont name="iconsafebox" /></template>
            <template #default>标记加密</template>
          </a-doption>
          <a-doption v-show='!isWebDav && (isShowBtn && dirtype !== "mypic"  || dirtype === "search")' class='danger' @click='() => menuTrashSelectFile(istree, false, isPic)'>
            <template #icon><IconFont name="icondelete" /></template>
            <template #default>放回收站</template>
          </a-doption>
          <a-dsubmenu v-if='dirtype !== "mypic" && (isAliyunAccount || isWebDav)' class='rightmenu' trigger='hover'>
            <template #default>
              <span class='arco-dropdown-option-icon'><IconFont name="iconrest" /></span>彻底删除
            </template>
            <template #content>
              <a-doption title='Ctrl+Shift+Delete' class='danger' @click='() => menuTrashSelectFile(istree, true, isPic)'>
                <template #default>删除后无法还原</template>
              </a-doption>
            </template>
          </a-dsubmenu>
        </template>
      </a-dsubmenu>

      <a-doption v-show="dirtype != 'video'"
                 @click='() => modalRename(istree, isselectedmulti, dirtype.includes("pic"))'>
        <template #icon><IconFont name="iconedit-square" /></template>
        <template #default>重命名</template>
      </a-doption>

      <a-doption v-show="!isPic" @click='() => modalShuXing(istree, dirtype.includes("pic"))'>
        <template #icon><IconFont name="iconshuxing" /></template>
        <template #default>属性</template>
      </a-doption>
      <a-dsubmenu v-if='!dirtype.includes("pic")'
                  id='rightpansubmore' class='rightmenu' trigger='hover'>
        <template #default>
          <div @click.stop='() => {}'>
            <span class='arco-dropdown-option-icon'>
              <IconFont name="icongengduo1" style='opacity: 0.8' />
            </span>
            更多
          </div>
        </template>
        <template #content>
          <a-doption
            v-show="isselected && !isselectedmulti && (dirtype == 'favorite' || dirtype == 'search' || dirtype == 'color' || dirtype == 'video')"
            @click='() => menuJumpToDir()'>
            <template #icon><IconFont name="icondakaiwenjianjia1" /></template>
            <template #default>打开位置</template>
          </a-doption>
          <a-doption v-show='isvideo' @click='() => menuVideoXBT()'>
            <template #icon><IconFont name="iconjietu" /></template>
            <template #default>雪碧图</template>
          </a-doption>
          <a-doption v-show='isShowBtn && isAliyunAccount && !isWebDav && !isThirdPartyDrive' type='text' size='small' tabindex='-1' title='Ctrl+M'
                     @click="() => menuFileEncTypeChange(istree)">
            <template #icon><IconFont name="iconsafebox" /></template>
            <template #default>标记加密</template>
          </a-doption>
          <a-doption v-show='isShowBtn && isAliyunAccount && !isWebDav' type='text' size='small' tabindex='-1' title='Ctrl+M'
                     @click="() => menuFileClearHistory(istree)">
            <template #icon><IconFont name="iconshipin" /></template>
            <template #default>清除历史</template>
          </a-doption>
          <a-doption v-show="isvideo" @click="() => menuDLNA()">
            <template #icon><IconFont name="icontouping2" /></template>
            <template #default>DLNA投屏</template>
          </a-doption>
          <a-doption v-show='isvideo' @click='() => menuM3U8Download()'>
            <template #icon><IconFont name="iconluxiang" /></template>
            <template #default>M3U8下载</template>
          </a-doption>
          <a-dsubmenu v-if='isDrive115Video' class='rightmenu' trigger='hover'>
            <template #default>
              <span class='arco-dropdown-option-icon'><IconFont name="iconshipin" /></span>115 转码
            </template>
            <template #content>
              <a-doption @click='() => menuDrive115VideoPush("vip_push")'>VIP 加速转码</a-doption>
              <a-doption @click='() => menuDrive115VideoPush("pay_push")'>枫叶加速转码</a-doption>
            </template>
          </a-dsubmenu>
          <a-doption v-show='isselected' @click='() => menuCopyFileName()'>
            <template #icon><IconFont name="iconlist" /></template>
            <template #default>复制文件名</template>
          </a-doption>
          <a-doption v-show='isselected && !isselectedmulti && !isCloudUser && !isThirdPartyDrive'
                     @click='() => menuCopyFileTree()'>
            <template #icon><IconFont name="iconnode-tree1" /></template>
            <template #default>复制目录树</template>
          </a-doption>
        </template>
      </a-dsubmenu>
    </template>
  </a-dropdown>
</template>
<style>
.ai-pro-badge { display: inline-flex; align-items: center; justify-content: center; border-radius: 999px; background: linear-gradient(135deg, #f59e0b, #f97316); color: #fff; font-weight: 700; line-height: 1; height: 14px; padding: 0 5px; font-size: 9px; vertical-align: middle; margin-left: 4px; }
</style>
