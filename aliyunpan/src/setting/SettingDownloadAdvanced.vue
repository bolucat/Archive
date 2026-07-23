<script setup lang="ts">
import useSettingStore from './settingstore'
import { Checkbox as AntdCheckbox } from 'ant-design-vue'
import { AriaApplyAdvancedOptions } from '../utils/aria2c'
import { fetchTrackerSource, normalizeTrackerText } from '../down/integration/tracker'
import { ref } from 'vue'
import message from '../utils/message'
import { t } from '../i18n'

const settingStore = useSettingStore()
const cb = (val: any) => settingStore.updateStore(val)
const trackerSyncing = ref(false)

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

const applyAriaOptions = () => AriaApplyAdvancedOptions().catch(() => {})
</script>

<template>
  <div class="settingcard">
    <div class="settinghead">{{ t('settings.aria.seeding') }}</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaKeepSeeding" @change="(e:any) => { cb({ ariaKeepSeeding: e.target.checked }); applyAriaOptions() }">
        {{ t('settings.aria.keepSeeding') }}
      </AntdCheckbox>
    </div>
    <div class="settingrow" v-show="!settingStore.ariaKeepSeeding">
      <span class="settinglabel">{{ t('settings.aria.seedRatio') }}</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaSeedRatio" :min="0" :step="0.5" :style="{ width: '90px' }" @update:model-value="(v: number) => { cb({ ariaSeedRatio: v || 0 }); applyAriaOptions() }" />
      <span class="settingitem">{{ t('settings.aria.times') }}</span>
      <span class="settinglabel" style="margin-left: 16px">{{ t('settings.aria.seedTime') }}</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaSeedTime" :min="0" :step="60" :style="{ width: '100px' }" @update:model-value="(v: number) => { cb({ ariaSeedTime: v || 0 }); applyAriaOptions() }" />
      <span class="settingitem">{{ t('settings.aria.minutes') }}</span>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaBtSaveMetadata" @change="(e:any) => cb({ ariaBtSaveMetadata: e.target.checked })">
        {{ t('settings.downloadAdvanced.saveBtMetadata') }}
      </AntdCheckbox>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaBtForceEncryption" @change="(e:any) => { cb({ ariaBtForceEncryption: e.target.checked }); applyAriaOptions() }">
        {{ t('settings.downloadAdvanced.forceBtEncryption') }}
      </AntdCheckbox>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaBtAutoDownloadContent" @change="(e:any) => cb({ ariaBtAutoDownloadContent: e.target.checked })">
        {{ t('settings.downloadAdvanced.autoDownloadBtContent') }}
      </AntdCheckbox>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.downloadAdvanced.trackerSync') }}</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaAutoSyncTracker" @change="(e:any) => cb({ ariaAutoSyncTracker: e.target.checked })">
        {{ t('settings.aria.autoSyncTrackerOnStart') }} {{ t('settings.aria.autoSyncEvery12h') }}
      </AntdCheckbox>
    </div>
    <div class="settingrow">
      <a-button :loading="trackerSyncing" size="small" type="outline" tabindex="-1" @click="handleSyncTrackers">{{ t('settings.aria.syncTracker') }}</a-button>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.downloadAdvanced.networkPorts') }}</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaEnableUpnp" @change="(e:any) => { cb({ ariaEnableUpnp: e.target.checked }); applyAriaOptions() }">
        {{ t('settings.downloadAdvanced.upnp') }}
      </AntdCheckbox>
    </div>
    <div class="settingrow">
      <span class="settinglabel">{{ t('settings.downloadAdvanced.btListenPort') }}</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaListenPort" :min="1024" :max="65535" :step="1" :style="{ width: '120px' }" @update:model-value="(v: number) => { cb({ ariaListenPort: v || 6881 }); applyAriaOptions() }" />
      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultValue') }}<span class="opred">6881</span>
            <hr />
            {{ t('settings.downloadAdvanced.btListenPortTip') }}<br />
            {{ t('settings.downloadAdvanced.restartAriaTip') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingrow">
      <span class="settinglabel">{{ t('settings.downloadAdvanced.dhtListenPort') }}</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaDhtListenPort" :min="1024" :max="65535" :step="1" :style="{ width: '120px' }" @update:model-value="(v: number) => { cb({ ariaDhtListenPort: v || 6881 }); applyAriaOptions() }" />
      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultValue') }}<span class="opred">6881</span>
            <hr />
            {{ t('settings.downloadAdvanced.dhtListenPortTip') }}<br />
            {{ t('settings.downloadAdvanced.restartAriaTip') }}
          </div>
        </template>
      </a-popover>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.downloadAdvanced.transferSettings') }}</div>
    <div class="settingrow">
      <span class="settinglabel">{{ t('settings.downloadAdvanced.globalUserAgent') }}</span>
      <a-textarea tabindex="-1" :model-value="settingStore.ariaUserAgent" :auto-size="{ minRows: 1, maxRows: 3 }" :style="{ width: '460px' }" :placeholder="t('settings.downloadAdvanced.globalUserAgentPlaceholder')" @update:model-value="(v: string) => cb({ ariaUserAgent: v })" />
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaContinueDownload" @change="(e:any) => cb({ ariaContinueDownload: e.target.checked })">
        {{ t('settings.downloadAdvanced.continueDownload') }}
      </AntdCheckbox>
      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultValue') }}<span class="opred">{{ t('settings.on') }}</span>
            <hr />
            {{ t('settings.downloadAdvanced.continueTipOn') }}<br />
            {{ t('settings.downloadAdvanced.continueTipOff') }}
          </div>
        </template>
      </a-popover>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.downloadAdvanced.taskRecovery') }}</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaResumeAllWhenLaunched" @change="(e:any) => cb({ ariaResumeAllWhenLaunched: e.target.checked })">
        {{ t('settings.aria.resumeOnLaunch') }}
      </AntdCheckbox>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaTaskNotification" @change="(e:any) => cb({ ariaTaskNotification: e.target.checked })">
        {{ t('settings.downloadAdvanced.taskNotification') }}
      </AntdCheckbox>
    </div>

<!--    <div class="settingspace"></div>-->
<!--    <div class="settinghead">上传限速（KB/s，0 = 不限）</div>-->
<!--    <div class="settingrow">-->
<!--      <a-input-number tabindex="-1" :model-value="settingStore.ariaMaxOverallUploadLimit" :min="0" :step="100" :style="{ width: '140px' }" @update:model-value="(v: number) => { cb({ ariaMaxOverallUploadLimit: v || 0 }); applyAriaOptions() }" />-->
<!--      <span class="settingitem">KB/s</span>-->
<!--    </div>-->
  </div>
</template>

<style></style>
