<script setup lang="ts">
import useSettingStore from './settingstore'
import AppCache from '../utils/appcache'
import MySwitch from '../layout/MySwitch.vue'
import { getUserData, openExternal } from '../utils/electronhelper'
import message from '../utils/message'
import { createProxyServer, getIPAddress } from '../utils/proxyhelper'
import { Sleep } from '../utils/format'
import { onMounted, ref } from 'vue'
import { t } from '../i18n'

const settingStore = useSettingStore()
const cb = (val: any) => {
  settingStore.updateStore(val)
}
const userData = getUserData()

const CHROMIUM_BLOCKED_PORTS = new Set([6665, 6666, 6667, 6668, 6669, 10080])
const getSafeProxyPort = () => {
  let port = 0
  do {
    port = Math.floor(Math.random() * (10000 - 2000 + 1) + 2000)
  } while (CHROMIUM_BLOCKED_PORTS.has(port))
  return port
}

const handleJumpPath = () => {
  openExternal(userData)
}
const handleResetHost = () => {
  if (settingStore.debugProxyHost.includes('127')) {
    let localIp = getIPAddress()
    cb({ debugProxyHost: localIp })
  } else {
    cb({ debugProxyHost: '127.0.0.1' })
  }
}

const handleResetPort = async () => {
  // 重启软件服务
  if (window.MainProxyServer) {
    const debugProxyPort = getSafeProxyPort()
    const loadingKey = 'proxyServer' + Date.now().toString()
    message.loading(t('settings.debug.restartLoading'), 60, loadingKey)
    await window.MainProxyServer.close()
    createProxyServer(debugProxyPort).catch(err => {
      message.error(t('settings.debug.restartFailed'), 3, loadingKey)
    }).then(async (debugProxyServer: any) => {
      window.MainProxyPort = debugProxyPort
      window.MainProxyServer = debugProxyServer
      window.MainProxyServer.on('close', async () => {
        await Sleep(2000)
        window.MainProxyServer = await createProxyServer(window.MainProxyPort)
      })
      cb({ debugProxyPort: debugProxyPort.toString() })
      await Sleep(2000)
      message.success(t('settings.debug.restartDone'), 3, loadingKey)
    })
  }
}

const cacheTotalBytes = ref(0)
const cacheLoading = ref(false)
const cacheClearing = ref(false)

const loadCacheStats = async () => {
  if (!(window as any).MsImageCacheStats) return
  cacheLoading.value = true
  try {
    const stats = await (window as any).MsImageCacheStats()
    cacheTotalBytes.value = stats.totalBytes ?? 0
  } finally {
    cacheLoading.value = false
  }
}

const handleClearCache = async () => {
  if (!(window as any).MsImageCacheClear) return
  cacheClearing.value = true
  try {
    await (window as any).MsImageCacheClear()
    await loadCacheStats()
    message.success(t('settings.debug.imageCacheCleared'))
  } catch {
    message.error(t('settings.debug.clearFailed'))
  } finally {
    cacheClearing.value = false
  }
}

const formatBytes = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

onMounted(loadCacheStats)
</script>

<template>
  <div class="settingcard">
    <div class="settinghead">{{ t('settings.debug.fileListLimit') }}</div>
    <div class="settingrow">
      <a-input-number tabindex="-1" :style="{ width: '252px' }" mode="button" :min="3000" :max="10000" :step="100"
                      :model-value="settingStore.debugFileListMax"
                      @update:model-value="cb({ debugFileListMax: $event })">
        <template #prefix> {{ t('settings.debug.showFirst') }}</template>
        <template #suffix> {{ t('settings.debug.fileCount') }}</template>
      </a-input-number>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            <span class="opred">{{ t('settings.debug.default3000') }}</span>
            <hr />
            {{ t('settings.debug.fileListTip') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.debug.specialListLimit') }}</div>
    <div class="settingrow">
      <div class="settingrow">
        <a-input-number tabindex="-1" :style="{ width: '252px' }" mode="button" :min="100"
                        :max="3000" :step="100" :model-value="settingStore.debugFavorListMax"
                        @update:model-value="cb({ debugFavorListMax: $event })">
          <template #prefix> {{ t('settings.debug.showFirst') }}</template>
          <template #suffix> {{ t('settings.debug.fileCount') }}</template>
        </a-input-number>
        <a-popover position="bottom">
          <IconFont name="iconbulb" />
          <template #content>
            <div>
              <span class="opred">{{ t('settings.debug.default500') }}</span>
              <hr />
              {{ t('settings.debug.specialListTip') }}
            </div>
          </template>
        </a-popover>
      </div>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.debug.transferRecordLimit') }}</div>
    <div class="settingrow">
      <div class="settingrow">
        <a-input-number tabindex="-1" :style="{ width: '252px' }" mode="button" :min="1000" :max="50000" :step="1000"
                        :model-value="settingStore.debugDownedListMax"
                        @update:model-value="cb({ debugDownedListMax: $event })">
          <template #prefix> {{ t('settings.debug.keepLast') }}</template>
          <template #suffix> {{ t('settings.debug.records') }}</template>
        </a-input-number>
        <a-popover position="bottom">
          <IconFont name="iconbulb" />
          <template #content>
            <div>
              <span class="opred">{{ t('settings.debug.default5000') }}</span>
              <hr />
              {{ t('settings.debug.recordTip') }}
            </div>
          </template>
        </a-popover>
      </div>
    </div>
  </div>

  <div class='settingcard' v-if="false">
    <a-alert banner type="warning">默认 不会产生任何 上传到服务器的数据</a-alert>
    <div class="settingspace"></div>
    <div class="settinghead">自动填写 分享链接提取码</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.yinsiLinkPassword" @update:value="cb({ yinsiLinkPassword: $event })">导入分享时
        尝试自动填写提取码
      </MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">关闭</span>
            <hr />
            有的分享链接需要填写提取码，如果你不知道提取码<br />
            开启后，有极小的几率可以自动填写<br />
            就是类似百度网盘助手自动填写提取码<br />
            <div class="hrspace"></div>
            <span class="oporg">注意：</span> 开启后会自动收集你 <span class="opblue">导入的</span> 分享链接的提取码
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">自动填写 在线解压密码</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.yinsiZipPassword" @update:value="cb({ yinsiZipPassword: $event })">在线解压时
        尝试自动填写解压密码
      </MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">关闭</span>
            <hr />
            有的压缩包在线解压需要填写密码，如果你不知道密码<br />
            开启后，有极小的几率可以自动填写<br />
            <div class="hrspace"></div>
            <span class="oporg">注意：</span> 开启后会自动收集你 <span class="opblue">在线解压</span> 的解压密码
          </div>
        </template>
      </a-popover>
    </div>
  </div>
  <div class="settingcard">
    <div class='settinghead'>{{ t('settings.debug.servicePort') }}</div>
    <a-popover position='bottom'>
      <IconFont name="iconbulb" />
      <template #content>
        <div>
          <span class='opred'>10000</span>
          <hr />
          {{ t('settings.debug.serviceTip') }}
        </div>
      </template>
    </a-popover>
    <div class='settingrow'>
      <a-input-search
        tabindex='-1'
        placeholder='127.0.0.1'
        hide-button
        style="width: fit-content"
        v-model.trim="settingStore.debugProxyHost"
        search-button
        :button-text="t('settings.debug.switchHost')"
        @search="handleResetHost"
        @update:model-value='cb({ debugProxyHost: $event })' />
    </div>
    <div class='settingrow'>
      <a-input-search
        tabindex='-1'
        placeholder='5000' hide-button
        style="width: fit-content"
        v-model.trim="settingStore.debugProxyPort"
        search-button
        :button-text="t('settings.debug.randomPort')"
        @search="handleResetPort"
        @update:model-value='cb({ debugProxyPort: $event })' />
    </div>
    <div class='settingspace'></div>
    <div class="settinghead">
      {{ t('settings.debug.cachePath') }}
      <span class="opblue" style="margin-left: 12px; padding: 0 12px">( {{ settingStore.debugDirSize }} )</span>
    </div>
    <div class="settingrow">
      <a-input tabindex='-1' :model-value='userData' placeholder='C:\Users\Username\AppData\Roaming\aliyunxby'
               :readonly='true' />
    </div>
    <div class="settingspace"></div>
    <div class='settingrow'>
      <a-button type='outline' size='small' @click='handleJumpPath'>
        {{ t('settings.debug.openLocation') }}
      </a-button>
      <a-popconfirm :content="t('settings.debug.confirmClearDb')" @ok="AppCache.aClearDir('db')">
        <a-button type="outline" size="small" tabindex="-1" status="danger" style="margin-right: 16px">{{ t('settings.debug.clearDb') }}
        </a-button>
      </a-popconfirm>

      <a-popconfirm :content="t('settings.debug.confirmClearCache')" @ok="AppCache.aClearDir('cache')">
        <a-button type="outline" size="small" tabindex="-1" status="danger" style="margin-right: 16px">{{ t('settings.debug.clearCache') }}
        </a-button>
      </a-popconfirm>
      <a-popconfirm :content="t('settings.debug.confirmReset')" @ok="AppCache.aClearDir('all')">
        <a-button type="outline" size="small" tabindex="-1" status="danger" style="margin-right: 16px">
          {{ t('settings.debug.resetAll') }}
        </a-button>
      </a-popconfirm>
    </div>
  </div>

  <div class="settingcard">
    <div class="settinghead">
      {{ t('settings.debug.mediaImageCache') }}
      <span v-if="cacheTotalBytes > 0" class="opblue" style="margin-left: 12px; padding: 0 12px">
        {{ formatBytes(cacheTotalBytes) }}
      </span>
      <a-spin v-if="cacheLoading" size="small" style="margin-left: 8px" />
    </div>
    <div class="settingrow">
      <a-button type="outline" size="small" tabindex="-1" :loading="cacheLoading" @click="loadCacheStats">
        {{ t('settings.debug.refreshStats') }}
      </a-button>
      <a-popconfirm :content="t('settings.debug.confirmClearImageCache')" @ok="handleClearCache">
        <a-button
          type="outline"
          size="small"
          status="danger"
          tabindex="-1"
          style="margin-left: 12px"
          :loading="cacheClearing"
        >
          {{ t('settings.debug.clearImageCache') }}
        </a-button>
      </a-popconfirm>
    </div>
  </div>
</template>

<style></style>
