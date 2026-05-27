<script setup lang="ts">
import { menuCopySelectedFile, menuCreatShare, menuDownload, menuTrashSelectFile } from '../topbtns/topbtn'
import { modalRename, modalShuXing } from '../../utils/modal'
import PanDAL from '../pandal'
import { usePanTreeStore, useAppStore } from '../../store'
import TreeStore from '../../store/treestore'
import { MediaScanner } from '../../utils/mediaScanner'
import MusicScanner from '../../utils/musicScanner'
import message from '../../utils/message'
import { computed } from 'vue'
import { isAliyunUser as isAliyunAccountUser, isBoxUser, isCloud123User, isDropboxUser, isOneDriveUser } from '../../aliapi/utils'

const istree = true
const pantreeStore = usePanTreeStore()
const appStore = useAppStore()
const mediaScanner = MediaScanner.getInstance()
const musicScanner = MusicScanner.getInstance()
const isCloudUser = computed(() => isCloud123User(pantreeStore.user_id || '') || pantreeStore.drive_id === 'cloud123')
const isAliyunAccount = computed(() => isAliyunAccountUser(pantreeStore.user_id || ''))
const isDropbox = computed(() => isDropboxUser(pantreeStore.user_id || '') || pantreeStore.drive_id === 'dropbox')
const isOneDrive = computed(() => isOneDriveUser(pantreeStore.user_id || '') || pantreeStore.drive_id === 'onedrive')
const isBox = computed(() => isBoxUser(pantreeStore.user_id || '') || pantreeStore.drive_id === 'box')
const isShareSupported = computed(() => props.inputselectType.includes('resource') || isDropbox.value || isOneDrive.value || isBox.value)

const props = defineProps({
  inputselectType: {
    type: String,
    required: true
  }
})

const handleRefresh = () => PanDAL.aReLoadOneDirToShow('', 'refresh', false)
const handleExpandAll = (isExpand: boolean) => {
  const drive_id = pantreeStore.drive_id
  const file_id = pantreeStore.selectDir.file_id
  const diridList = (() => {
    const result: string[] = []
    const visited = new Set<string>()
    const stack = [file_id]
    while (stack.length > 0) {
      const current = stack.pop() as string
      const children = TreeStore.GetDirChildDirID(drive_id, current)
      for (let i = 0; i < children.length; i++) {
        const child = children[i]
        if (visited.has(child)) continue
        visited.add(child)
        result.push(child)
        stack.push(child)
      }
    }
    return result
  })()
  pantreeStore.mTreeExpandAll(diridList, isExpand)
}

const buildSelectedFolder = () => {
  const selectDir = pantreeStore.selectDir
  if (!selectDir || !selectDir.file_id) {
    message.warning('请先选择要扫描的文件夹')
    return null
  }
  const folder = {
    __v_skip: true,
    drive_id: pantreeStore.drive_id,
    file_id: selectDir.file_id,
    parent_file_id: selectDir.parent_file_id || '',
    name: selectDir.name,
    namesearch: (selectDir.name || '').toLowerCase(),
    ext: '',
    mime_type: '',
    mime_extension: '',
    category: 'folder',
    icon: 'iconfolder',
    file_count: 0,
    size: 0,
    sizeStr: '',
    time: Date.now(),
    timeStr: new Date().toLocaleString(),
    starred: false,
    isDir: true,
    thumbnail: '',
    path: (selectDir as any).path || ''
  } as any
  return folder
}

// 扫描视频
const handleScanVideo = async () => {
  const folder = buildSelectedFolder()
  if (!folder) return
  if (mediaScanner.isCurrentlyScanning) {
    message.warning('正在扫描中，请稍后...')
    return
  }
  try {
    message.info(`开始扫描文件夹 "${folder.name}" 的视频文件`)
    appStore.toggleTab('media')
    await mediaScanner.scanFolder(folder, pantreeStore.drive_id)
  } catch (error) {
    console.error('视频扫描失败:', error)
    message.error('视频扫描失败，请稍后重试')
  }
}

// 扫描音频
const handleScanAudio = async () => {
  const folder = buildSelectedFolder()
  if (!folder) return
  if (musicScanner.isScanning) {
    message.warning('音频扫描进行中，请稍后...')
    return
  }
  const userId = pantreeStore.user_id || ''
  if (!userId) {
    message.error('未识别到当前账号，无法扫描')
    return
  }
  try {
    message.info(`开始扫描文件夹 "${folder.name}" 的音频文件`)
    appStore.toggleTab('music')
    const res = await musicScanner.scanFolder(folder, userId)
    message.success(`音频扫描完成：收录 ${res.found} 首`)
  } catch (error) {
    console.error('音频扫描失败:', error)
    message.error('音频扫描失败，请稍后重试')
  }
}

// 检查是否选中了有效的文件夹
const isSelectedFolder = computed(() => {
  return pantreeStore.selectDir && pantreeStore.selectDir.file_id && pantreeStore.selectDir.file_id !== ''
})

</script>

<template>
  <a-dropdown id="leftpanmenu" class="rightmenu" :popup-visible="true" style="z-index: -1; left: -200px; opacity: 0">
    <template #content>
      <a-dsubmenu id="leftpansubzhankai" class="rightmenu" trigger="hover">
        <template #default>
          <div @click.stop="() => {}">
            <span class="arco-dropdown-option-icon"><IconFont name="iconfenzhi1" /></span>目录
          </div>
        </template>
        <template #content>
          <a-doption @click="handleRefresh">
            <template #icon> <IconFont name="iconreload-1-icon" /> </template>
            <template #default>刷新</template>
          </a-doption>
          <a-doption @click="() => handleExpandAll(true)">
            <template #icon> <IconFont name="iconArrow-Down2" /> </template>
            <template #default>展开全部</template>
          </a-doption>
          <a-doption @click="() => handleExpandAll(false)">
            <template #icon> <IconFont name="iconArrow-Right2" /> </template>
            <template #default>折叠全部</template>
          </a-doption>
        </template>
      </a-dsubmenu>
      <a-doption @click="() => menuDownload(istree)">
        <template #icon> <IconFont name="icondownload" /> </template>
        <template #default>下载</template>
      </a-doption>
      <a-doption v-show="isShareSupported"
                 @click="() => menuCreatShare(istree, 'pan', 'resource_root')">
        <template #icon><IconFont name="iconfenxiang" /></template>
        <template #default>分享</template>
      </a-doption>
      <a-doption v-if="isAliyunAccount" @click="() => menuCreatShare(istree, 'pan', 'backup_root')">
        <template #icon><IconFont name="iconrss" /></template>
        <template #default>快传</template>
      </a-doption>

      <!-- 扫描视频 / 扫描音频 -->
      <a-doption @click="handleScanVideo">
        <template #icon><IconFont name="iconshipin" /></template>
        <template #default>扫描视频</template>
      </a-doption>
      <a-doption @click="handleScanAudio">
        <template #icon><IconFont name="iconmusic" /></template>
        <template #default>扫描音频</template>
      </a-doption>

      <a-dsubmenu id="leftpansubmove" class="rightmenu" trigger="hover">
        <template #default>
          <div @click.stop="() => {}">
            <span class="arco-dropdown-option-icon"><IconFont name="iconmoveto" style="opacity: 0.8" /></span>移动
          </div>
        </template>
        <template #content>
          <a-doption @click="() => menuCopySelectedFile(istree, 'cut')">
            <template #icon> <IconFont name="iconscissor" /> </template>
            <template #default>移动到...</template>
          </a-doption>
          <a-doption @click="() => menuCopySelectedFile(istree, 'copy')">
            <template #icon> <IconFont name="iconcopy" /> </template>
            <template #default>复制到...</template>
          </a-doption>
          <a-doption class="danger" @click="() => menuTrashSelectFile(istree, false)">
            <template #icon> <IconFont name="icondelete" /> </template>
            <template #default>回收站</template>
          </a-doption>
        </template>
      </a-dsubmenu>

      <a-doption @click='() => modalRename(istree, false, false)'>
        <template #icon><IconFont name="iconedit-square" /></template>
        <template #default>重命名</template>
      </a-doption>

      <a-doption @click='() => modalShuXing(istree)'>
        <template #icon><IconFont name="iconshuxing" /></template>
        <template #default>属性</template>
      </a-doption>
    </template>
  </a-dropdown>
</template>
<style></style>
