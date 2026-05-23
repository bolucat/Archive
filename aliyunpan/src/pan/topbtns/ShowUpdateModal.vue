<script setup lang="ts">

import { modalCloseAll } from '../../utils/modal'
import { nextTick, PropType, ref } from 'vue'
import { IServerVerData } from '../../aliapi/server'
import MarkdownIt from 'markdown-it'
import { getAppNewPath, getResourcesPath, getUserDataPath, openExternal } from '../../utils/electronhelper'
import fs, { existsSync, rmSync, writeFile } from 'fs'
import message from '../../utils/message'
import axios, { AxiosResponse } from 'axios'
import { Sleep } from '../../utils/format'
import { execFile, SpawnOptions } from 'child_process'
import { shell } from 'electron'
import path from 'path'
import { Progress as AntdProgress } from 'ant-design-vue'
import useFootStore from '../../store/footstore'

const props = defineProps({
  visible: {
    type: Boolean,
    required: true
  },
  verData: {
    type: Object as PropType<IServerVerData>,
    required: true
  }
})
const okLoading = ref(false)
const percent = ref(0)
const loaded = ref(0)
const footStore = useFootStore()

const handleOpen = async () => {
  const markdown = new MarkdownIt({
    html: false,
    linkify: true,
    typographer: true
  })
  await nextTick(() => {
    const el = document.getElementById('markdown-content')
    if (el) el.innerHTML = markdown.render(props.verData.verInfo || '')
  })
}

const handleHide = () => {
  if (okLoading.value) okLoading.value = false
  percent.value = 0
  loaded.value = 0
  footStore.mSaveUpdateDownloadProgress(0)
  if (props.verData.verName) {
    let resourcesPath = getResourcesPath(props.verData.verName)
    if (existsSync(resourcesPath)) {
      rmSync(resourcesPath, { force: true })
    }
  }
  modalCloseAll()
}
const handleOK = async () => {
  let { version, verName, verUrl, verHtml } = props.verData
  let isHot = props.verData.fileExt.includes('asar')
  if (verUrl && window.platform !== 'linux') {
    okLoading.value = true
    // 下载安装
    const flag = await AutoDownload(verUrl, verName, verHtml, isHot)
    okLoading.value = false
    // 更新本地版本号
    if (flag && version) {
      const localVersion = getResourcesPath('localVersion')
      if (localVersion) {
        writeFile(localVersion, version, async (err) => {
          if (err) {
            message.error('更新本地版本号失败，请检查【Resources文件夹】是否有写入权限【不要安装到系统盘】', 5)
          } else {
            message.info('热更新完毕，重新打开应用...', 0)
            await Sleep(500)
            window.WebToElectron({ cmd: 'relaunch' })
          }
        })
      }
    } else {
      percent.value = 0
      message.error('新版本下载失败，请前往github下载最新版本', 8)
      openExternal(verHtml)
    }
  } else {
    openExternal(verHtml)
  }
}

const AutoDownload = async (url: string, name: string, html_url: string, hot: boolean) => {
  const downPath = hot ? getAppNewPath() : getUserDataPath(name)
  if (!hot && existsSync(downPath) && fs.statSync(downPath).size == props.verData.fileSize) {
    await autoInstallNewVersion(downPath)
    return true
  } else {
    await fs.promises.rm(downPath, { force: true })
  }
  return axios
    .get(url, {
      withCredentials: false,
      responseType: 'arraybuffer',
      timeout: 60000,
      headers: {
        'Cache-Control': 'no-cache',
        Pragma: 'no-cache',
        Expires: '0'
      },
      onDownloadProgress: (progressEvent) => {
        let total = props.verData.fileSize
        loaded.value = progressEvent.loaded
        if (total) {
          let progress = (loaded.value > 0) ? Math.ceil(loaded.value / (total / 100)) : 0
          percent.value = (progress > 100) ? 100 : progress
          footStore.mSaveUpdateDownloadProgress(percent.value)
        }
      }
    })
    .then(async (response: AxiosResponse) => {
      writeFile(downPath, Buffer.from(response.data), (err) => {
        if (err) {
          return false
        }
      })
      footStore.mSaveUpdateDownloadProgress(0)
      if (!hot) {
        await autoInstallNewVersion(downPath)
      }
      return true
    })
    .catch(() => {
      rmSync(downPath, { force: true })
      return false
    })
}

const autoInstallNewVersion = async (resourcesPath: string) => {
  // 自动安装
  const options: SpawnOptions = { shell: true, windowsVerbatimArguments: true }
  const subProcess = execFile(`${resourcesPath}`, options)
  if (subProcess.pid && process.kill(subProcess.pid, 0)) {
    await Sleep(2000)
    window.WebToElectron({ cmd: 'exit' })
  } else {
    message.info('安装失败，请前往文件夹手动安装', 5)
    const resources = getResourcesPath('')
    await shell.openPath(path.join(resources, '/'))
  }
}
</script>

<template>
  <a-modal :visible='visible'
           modal-class='modalclass updatemodal'
           :unmount-on-close='true'
           :mask-closable='false'
           :closable="false"
           @cancel='handleHide'
           @before-open='handleOpen'>
    <template #title>
      <span class='vermodaltitle' style="max-width: 540px">
        <i class='iconfont iconyibu verupdate-icon'></i>
        发现新版本<span class='vertip'>{{ verData.version }}</span>
      </span>
    </template>
    <div class='vermodalbody'>
      <div id='markdown-content' />
    </div>
    <template #footer>
      <div class='modalfoot'>
        <AntdProgress
          v-show="percent > 0"
          size="small"
          style="width: 250px;"
          status='active'
          :stroke-color="{
              '0%': '#ffba7a',
              '8.56%': '#ff74c7',
              '26.04%': '#637dff',
              '100%': 'rgba(99, 125, 255, 0.2)',
            }"
          :percent="percent">
          <template #format="percent">
            {{ `${percent}%(${loaded}/${props.verData.fileSize})` }}
          </template>
        </AntdProgress>
        <div style='flex-grow: 1'></div>
        <a-button v-if='!okLoading' type='outline' size='small' @click='handleHide'>取消</a-button>
        <a-button type='primary' size='small' :loading='okLoading' @click='handleOK'>更新</a-button>
      </div>
    </template>
  </a-modal>
</template>

<style scoped>
.vermodaltitle {
  display: flex;
  align-items: center;
  line-height: 48px;
}

.vermodalbody {
  width: 540px;
  max-height: calc(70vh - 100px);
  flex-direction: column;
  justify-content: center;
  align-items: center;
  overflow-x: hidden;
  padding: 0 16px 16px 16px !important;
}

.verupdate-icon {
  font-size: 20px;
  color: rgb(40, 104, 240);
  margin-right: 8px;
  line-height: 1;
}

.vertip {
  padding-left: 12px;
  color: rgb(40, 104, 240);
  flex-grow: 1;
}
</style>