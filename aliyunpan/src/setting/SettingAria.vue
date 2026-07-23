<script setup lang="ts">
import { ref } from 'vue'
import useSettingStore from './settingstore'
import { AriaChangeToLocal, AriaChangeToRemote, AriaTest, AriaApplyAdvancedOptions } from '../utils/aria2c'
import { fetchTrackerSource, normalizeTrackerText } from '../down/integration/tracker'
import message from '../utils/message'
import { t } from '../i18n'

import { Checkbox as AntdCheckbox } from 'ant-design-vue'

const settingStore = useSettingStore()
const cb = (val: any) => {
  settingStore.updateStore(val)
}
const ariaState = ref(settingStore.ariaState)
const ariaLoading = ref(settingStore.ariaLoading)
const ariaSavePath = ref(settingStore.ariaSavePath)
const ariaUrl = ref(settingStore.ariaUrl)
const ariaPwd = ref(settingStore.ariaPwd)
const trackerSyncing = ref(false)

const handleAriaConn = () => {
  ariaSavePath.value = ariaSavePath.value.trim()
  if (!ariaSavePath.value || (ariaSavePath.value.indexOf('/') < 0 && ariaSavePath.value.indexOf('\\') < 0)) {
    message.error(t('settings.aria.savePathRequired'))
    return
  }

  let val2 = ariaUrl.value
  val2 = val2.replaceAll('：', ':')
  if (val2.indexOf('://') > 0) val2 = val2.substring(val2.indexOf('://') + 3)
  if (val2.indexOf('/js') > 0) val2 = val2.substring(0, val2.indexOf('/js'))
  ariaUrl.value = val2.trim()
  if (!ariaUrl || ariaUrl.value.indexOf(':') < 0) {
    message.error(t('settings.aria.addressRequired'))
    return
  }

  ariaPwd.value = ariaPwd.value.trim()
  if (!ariaPwd) {
    message.error(t('settings.aria.secretRequired'))
    return
  }

  settingStore.updateStore({ ariaSavePath: ariaSavePath.value, ariaUrl: ariaUrl.value, ariaPwd: ariaPwd.value, ariaLoading: true })



  try {
    const host = ariaUrl.value.split(':')[0]
    const port = parseInt(ariaUrl.value.split(':')[1])
    const secret = ariaPwd.value

    AriaTest(settingStore.ariaHttps, host, port, secret).then((issuccess: boolean) => {
      if (issuccess) {
        settingStore.updateStore({ ariaState: 'remote' })
        AriaChangeToRemote().then((isOnline: boolean | undefined) => {
          settingStore.ariaLoading = false
          if (isOnline == true) {
            message.success(t('settings.aria.remoteSuccess'))
          } else if (isOnline == undefined) {
            message.warning(t('settings.aria.remoteBusy'))
          } else {
            message.error(t('settings.aria.remoteFailed'))
          }
        })
      } else {
        settingStore.ariaLoading = false
      }
    })
  } catch (e: any) {
    settingStore.ariaLoading = false
    message.error(t('settings.aria.dataFormatError') + e.message)
  }
}
const handleAriaOff = (tip: boolean) => {
  settingStore.updateStore({ ariaState: 'local', ariaLoading: true })

  AriaChangeToLocal()
    .then((isOnline: boolean) => {
      settingStore.ariaLoading = false
      if (tip) {
        if (isOnline) message.warning(t('settings.aria.localConnected'))
        else message.error(t('settings.aria.localFailed'))
      }
    })
    .catch(() => {
      settingStore.ariaLoading = false
      message.error(t('settings.aria.localFailed'))
    })
}

const handleSyncTrackers = async () => {
  trackerSyncing.value = true
  try {
    const texts = await Promise.all(
      settingStore.ariaTrackerSources.map((url) => fetchTrackerSource(url))
    )
    const ariaBtTracker = normalizeTrackerText(texts.join('\n'))
    settingStore.updateStore({ ariaBtTracker })
    await AriaApplyAdvancedOptions()
    message.success(t('settings.aria.trackerSynced'))
  } catch (error: any) {
    message.error(error?.message || t('settings.aria.trackerSyncFailed'))
  } finally {
    trackerSyncing.value = false
  }
}
</script>

<template>
  <div class="settingcard">
    <a-alert banner>{{ t('settings.aria.remoteDownloadTip') }}</a-alert>
    <div class="settingspace"></div>

    <div class="settinghead">{{ t('settings.aria.savePath') }}</div>
    <div class="settingrow">
      <a-input tabindex="-1" :disabled="!settingStore.AriaIsLocal" :style="{ width: '300px' }" :placeholder="t('settings.aria.savePathPlaceholder')" v-model:model-value="ariaSavePath" />
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.aria.savePathTip') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.aria.rpcAddress') }}</div>
    <div class="settingrow">
      <a-input tabindex="-1" :disabled="!settingStore.AriaIsLocal" :style="{ width: '300px' }" :placeholder="t('settings.aria.rpcPlaceholder')" v-model:model-value="ariaUrl">
        <template #prefix> ws:// </template>
        <template #suffix> /jsonrpc </template>
      </a-input>

      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.aria.rpcTip') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.aria.secret') }}</div>
    <div class="settingrow">
      <a-input tabindex="-1" :disabled="!settingStore.AriaIsLocal" :style="{ width: '300px' }" :placeholder="t('settings.aria.secretPlaceholder')" v-model:model-value="ariaPwd" />
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.aria.secretTip') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.aria.ssl') }}</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaHttps" @change="(e:any)=>cb({ ariaHttps: e.target.checked })">{{ t('settings.aria.useSsl') }}</AntdCheckbox>

      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            <span class="opred">{{ t('settings.aria.defaultUnchecked') }}</span><br />
            {{ t('settings.aria.sslTip') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.aria.status') }}</div>
    <div class="settingrow" v-show="settingStore.AriaIsLocal">
      <a-button type="outline" size="small" tabindex="-1" :loading="settingStore.ariaLoading" @click="handleAriaConn">{{ t('settings.aria.localModeButton') }}</a-button>
    </div>
    <div class="settingrow" v-show="!settingStore.ariaLoading && settingStore.AriaIsLocal">
      <a-typography-text type="secondary">{{ t('settings.aria.localModeTip') }}</a-typography-text>
    </div>

    <div class="settingrow" v-show="!settingStore.AriaIsLocal">
      <a-button type="primary" size="small" tabindex="-1" :loading="settingStore.ariaLoading" @click="handleAriaOff(false)">{{ t('settings.aria.remoteModeButton') }}</a-button>
    </div>
    <div class="settingrow" v-show="!settingStore.ariaLoading && !settingStore.AriaIsLocal">
      <a-typography-text type="secondary">{{ t('settings.aria.remoteModeTip') }}</a-typography-text>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">BT Tracker</div>
    <div class="settingrow">
      <a-textarea
        :model-value="settingStore.ariaBtTracker"
        :auto-size="{ minRows: 3, maxRows: 6 }"
        :placeholder="t('settings.aria.trackerPlaceholder')"
        @update:model-value="(v: string) => cb({ ariaBtTracker: v })"
        style="width: 460px; font-size: 12px"
      />
    </div>
    <div class="settingrow">
      <a-button :loading="trackerSyncing" size="small" type="outline" tabindex="-1" @click="handleSyncTrackers">{{ t('settings.aria.syncTracker') }}</a-button>
      <span class="settingitem">{{ t('settings.aria.autoSyncEvery12h') }}</span>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaAutoSyncTracker" @change="(e:any)=>cb({ ariaAutoSyncTracker: e.target.checked })">{{ t('settings.aria.autoSyncTrackerOnStart') }}</AntdCheckbox>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.aria.uploadLimit') }}</div>
    <div class="settingrow">
      <a-input-number
        tabindex="-1"
        :model-value="settingStore.ariaMaxOverallUploadLimit"
        :min="0"
        :step="100"
        :style="{ width: '140px' }"
        @update:model-value="(v: number) => cb({ ariaMaxOverallUploadLimit: v || 0 })"
      />
      <span class="settingitem">KB/s</span>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.aria.seeding') }}</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaKeepSeeding" @change="(e:any)=>cb({ ariaKeepSeeding: e.target.checked })">{{ t('settings.aria.keepSeeding') }}</AntdCheckbox>
    </div>
    <div class="settingrow" v-show="!settingStore.ariaKeepSeeding">
      <span class="settinglabel">{{ t('settings.aria.seedRatio') }}</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaSeedRatio" :min="0" :step="0.5" :style="{ width: '100px' }" @update:model-value="(v: number) => cb({ ariaSeedRatio: v || 0 })" />
      <span class="settingitem">{{ t('settings.aria.times') }}</span>
      <span class="settinglabel" style="margin-left: 16px">{{ t('settings.aria.seedTime') }}</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaSeedTime" :min="0" :step="60" :style="{ width: '100px' }" @update:model-value="(v: number) => cb({ ariaSeedTime: v || 0 })" />
      <span class="settingitem">{{ t('settings.aria.minutes') }}</span>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaResumeAllWhenLaunched" @change="(e:any)=>cb({ ariaResumeAllWhenLaunched: e.target.checked })">{{ t('settings.aria.resumeOnLaunch') }}</AntdCheckbox>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.aria.browserIntegration') }}</div>
    <div class="settingrow">
      <span class="settinglabel">{{ t('settings.aria.rpcUrl') }}</span>
      <a-input readonly :model-value="`http://localhost:${settingStore.ariaRpcListenPort}/jsonrpc`" :style="{ width: '260px' }" />
    </div>
    <div class="settingrow">
      <span class="settinglabel">{{ t('settings.aria.token') }}</span>
      <a-input readonly :model-value="settingStore.ariaRpcSecret" :style="{ width: '260px' }" />
    </div>
    <div class="settingrow">
      <a-alert type="info" :content="t('settings.aria.extensionTip')" />
    </div>
  </div>
</template>

<style></style>
