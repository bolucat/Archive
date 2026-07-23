<script setup lang="ts">
import { computed } from 'vue'
import message from '../utils/message'
import DebugLog from '../utils/debuglog'
import { useLogStore, useWinStore } from '../store'
import { copyToClipboard } from '../utils/electronhelper'
import { t } from '../i18n'

const logStore = useLogStore()
const winStore = useWinStore()

const logHeight = computed(() => winStore.height - 316)

const handleSaveLogRefresh = () => {
  DebugLog.aLoadFromDB()
}

const handleSaveLogClear = () => {
  DebugLog.mSaveLogClear()
}

const handleSaveLogCopy = () => {
  let logstr = ''
  const logList = DebugLog.logList
  for (let i = 0, maxi = logList.length; i < maxi; i++) {
    const item = logList[i]
    logstr += item.logtime + ' : ' + item.logtype + ' : ' + item.logmessage + '\n'
  }
  copyToClipboard(logstr)
  message.success(t('settings.log.copied'))
}
</script>

<template>
  <div class="settingcard">
    <div class="settings-log-header">
      <div>
        <div class="settings-log-kicker">Diagnostics</div>
        <div class="settinghead">{{ t('settings.logs') }}</div>
      </div>
      <div class="settings-log-caption">{{ t('settings.log.caption') }}</div>
    </div>
    <a-list
      :bordered="false"
      :max-height="logHeight"
      :style="{ height: logHeight + 'px' }"
      :data="DebugLog.logList"
      class="loglist"
      :data-refresh="logStore.logTime"
      :virtual-list-props="{
        height: logHeight,
        threshold: 50
      }">
      <template #item="{ item, index }">
        <a-list-item :key="index">
          <a-typography-text :type="item.logtype"> [{{ item.logtime }}] </a-typography-text>
          {{ item.logmessage }}
        </a-list-item>
      </template>
    </a-list>

    <div class="settingspace"></div>
    <div class="settingrow">
      <a-button type="outline" size="small" @click="handleSaveLogRefresh">{{ t('common.refresh') }}</a-button>
      <a-button type="outline" size="small" @click="handleSaveLogClear">{{ t('settings.log.clear') }}</a-button>
      <a-button type="outline" size="small" @click="handleSaveLogCopy">{{ t('settings.log.copy') }}</a-button>
    </div>
  </div>
</template>

<style>
.loglist {
  box-sizing: content-box;
  overflow: hidden;
  border: 1px solid rgba(120, 138, 165, 0.2);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.38);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
}
.settings-log-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 12px;
}
.settings-log-kicker {
  display: inline-flex;
  margin-bottom: 8px;
  padding: 4px 10px;
  border-radius: 999px;
  background: rgba(88, 130, 255, 0.12);
  color: var(--color-primary-6);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.settings-log-caption {
  color: var(--color-text-3);
  font-size: 13px;
  line-height: 1.6;
  text-align: right;
}
.loglist .arco-list {
  height: 100%;
  overflow-y: hidden !important;
}
.loglist .arco-list-item {
  padding: 8px 12px !important;
  font-size: 12px;
  color: var(--color-text-2);
}
body[arco-theme='dark'] #xbybody .loglist .arco-list-item {
  color: rgba(232, 238, 249, 0.88);
}
body[arco-theme='dark'] #xbybody .loglist .arco-list-item .arco-typography {
  color: inherit;
}
.loglist .arco-list-item-content {
  user-select: text;
  -webkit-user-drag: none;
}
body[arco-theme='dark'] #xbybody .loglist {
  border-color: rgba(140, 158, 183, 0.18);
  background: #05070a;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}
body[arco-theme='dark'] #xbybody .loglist .arco-list,
body[arco-theme='dark'] #xbybody .loglist .arco-list-content,
body[arco-theme='dark'] #xbybody .loglist .arco-list-content-wrapper,
body[arco-theme='dark'] #xbybody .loglist .arco-list-item,
body[arco-theme='dark'] #xbybody .loglist .arco-virtual-list,
body[arco-theme='dark'] #xbybody .loglist .arco-virtual-list-list,
body[arco-theme='dark'] #xbybody .loglist .arco-empty {
  background: transparent !important;
}
body[arco-theme='dark'] #xbybody .loglist .arco-empty {
  color: rgba(232, 238, 249, 0.42);
}
body[arco-theme='dark'] #xbybody .loglist .arco-empty-image {
  opacity: 0.42;
}
body[arco-theme='dark'] #xbybody .settings-log-kicker {
  background: rgba(120, 160, 255, 0.2);
  color: #dbe6ff;
}
body[arco-theme='dark'] #xbybody .settings-log-caption {
  color: rgba(236, 242, 255, 0.62);
}
</style>
