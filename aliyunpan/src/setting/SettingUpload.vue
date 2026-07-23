<script setup lang="ts">
import useSettingStore from './settingstore'
import MySwitch from '../layout/MySwitch.vue'
import MyTags from '../layout/MyTags.vue'
import { t } from '../i18n'

const settingStore = useSettingStore()
const cb = (val: any) => {
  settingStore.updateStore(val)
}
</script>

<template>
  <div class="settingcard">
    <div class="settinghead">{{ t('settings.upload.maxParallel') }}</div>
    <div class="settingrow">
      <a-select tabindex="-1" :style="{ width: '252px' }" :model-value="settingStore.uploadFileMax" :popup-container="'#SettingDiv'" @update:model-value="cb({ uploadFileMax: $event })">
        <a-option :value="1">
          {{ t('settings.upload.parallel1') }}
          <template #suffix>{{ t('settings.upload.largeFiles') }}</template>
        </a-option>
        <a-option :value="3">{{ t('settings.upload.parallel3') }}</a-option>
        <a-option :value="5">
          {{ t('settings.upload.parallel5') }}
          <template #suffix>{{ t('settings.upload.recommended') }}</template>
        </a-option>
        <a-option :value="10">{{ t('settings.upload.parallel10') }}</a-option>
        <a-option :value="20">{{ t('settings.upload.parallel20') }}</a-option>
        <a-option :value="30">{{ t('settings.upload.parallel30') }}<template #suffix>{{ t('settings.upload.manySmallFiles') }}</template></a-option>
        <a-option :value="50">{{ t('settings.upload.parallel50') }}</a-option>
      </a-select>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.upload.globalSpeed') }}</div>
    <div class="settingrow" style="display: flex; align-items: center">
      <a-input-number
        tabindex="-1"
        :style="{ width: '128px' }"
        mode="button"
        :min="0"
        :max="settingStore.uploadGlobalSpeedM == 'MB' ? 100 : 999"
        :step="settingStore.uploadGlobalSpeedM == 'MB' ? 1 : 40"
        :model-value="settingStore.uploadGlobalSpeed"
        @update:model-value="cb({ uploadGlobalSpeed: $event })">
      </a-input-number>
      <div style="height: 32px; border-left: 1px solid var(--color-neutral-3)"></div>
      <a-radio-group type="button" tabindex="-1" :model-value="settingStore.uploadGlobalSpeedM" @update:model-value="cb({ uploadGlobalSpeedM: $event, uploadGlobalSpeed: 0 })">
        <a-radio tabindex="-1" value="MB">MB/s</a-radio>
        <a-radio tabindex="-1" value="KB">KB/s</a-radio>
      </a-radio-group>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div :style="{ width: '360px' }">
            {{ t('settings.defaultValue') }}<span class="opred">0 ({{ t('settings.upload.unlimitedFullSpeed') }})</span>
            <hr />
            <span class="opred">0-100MB/s</span> {{ t('settings.upload.speedTip100') }}<br />
            <span class="opred">0-999KB/s</span> {{ t('settings.upload.speedTipKb') }}<br />
            {{ t('settings.upload.speedTip') }}
          </div>
        </template>
      </a-popover>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.upload.instantMode') }}</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.downUploadBreakFile" @update:value="cb({ downUploadBreakFile: $event })">{{ t('settings.upload.instantModeSwitch') }}</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultOff') }}
            <hr />
            {{ t('settings.upload.instantModeTip1') }}<br />
            {{ t('settings.upload.instantModeTip2') }}
          </div>
        </template>
      </a-popover>
    </div>
  </div>

  <div class="settingcard">
    <div class="settinghead">{{ t('settings.uploadConflict') }}</div>
    <div class="settingrow">
      <a-select tabindex="-1" :style="{ width: '252px' }" :model-value="settingStore.downUploadWhatExist" @update:model-value="cb({ downUploadWhatExist: $event })">
        <a-option value="ignore">{{ t('settings.uploadConflictIgnore') }}</a-option>
        <a-option value="overwrite">{{ t('settings.uploadConflictOverwrite') }}</a-option>
        <a-option value="auto_rename">{{ t('settings.uploadConflictRename') }}</a-option>
        <a-option value="refuse">{{ t('settings.uploadConflictRefuse') }}</a-option>
      </a-select>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.upload.autoShutdown') }}</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.downAutoShutDown > 0"
                @update:value="cb({ downAutoShutDown: $event ? 1 : 0 })">
        {{ t('settings.upload.autoShutdownSwitch') }}
      </MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultOff') }}
            <hr />
            {{ t('settings.upload.autoShutdownTip1') }}<br />
            {{ t('settings.upload.autoShutdownTip2') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.upload.finishSound') }}</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.downFinishAudio" @update:value="cb({ downFinishAudio: $event })">{{ t('settings.upload.finishSoundSwitch') }}</MySwitch>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.upload.smallFilesFirst') }}</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.downSmallFileFirst" @update:value="cb({ downSmallFileFirst: $event })">{{ t('settings.upload.smallFilesFirstSwitch') }}</MySwitch>
      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultOff') }}
            <hr />
            {{ t('settings.upload.smallFilesFirstTip') }}
          </div>
        </template>
      </a-popover>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.upload.taskbarProgress') }}</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.downSaveShowPro" @update:value="cb({ downSaveShowPro: $event })">{{ t('settings.upload.taskbarProgressSwitch') }}</MySwitch>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">
      {{ t('settings.upload.preFilter') }}
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.upload.preFilterIntro') }}
            <hr />
            {{ t('settings.upload.preFilterRule1') }}<br />
            {{ t('settings.upload.preFilterRule2') }}<br />
            {{ t('settings.upload.preFilterRule3') }}<br />
            {{ t('settings.upload.preFilterRule4') }}<span class="oporg">{{ t('settings.upload.preFilterRule4Warning') }}</span> <br />
            {{ t('settings.upload.preFilterRule5') }} <br />
            <div class="hrspace"></div>
            <div class="hrspace"></div>
            <a-typography-text mark> 　if(　fileName.toLower().endWith('<span class="opred">.mp3</span>')　) break 　</a-typography-text>
            <br />
            {{ t('settings.upload.preFilterExamplePrefix') }}<span class="opred">.mp3</span>{{ t('settings.upload.preFilterExample1') }}<br />
            {{ t('settings.upload.preFilterExamplePrefix') }}<span class="opred">001.ppt.txt</span>{{ t('settings.upload.preFilterExample2') }}
            <div class="hrspace"></div>
            <div class="hrspace"></div>
            {{ t('settings.upload.preFilterDefaults') }}<span class="opred">thumbs.db</span>, <span class="opred">desktop.ini</span>, <span class="opred">.ds_store</span>, <span class="opred">.td</span>, <span class="opred">.downloading</span>
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingrow">
      <MyTags :value="settingStore.downIngoredList" :maxlen="20" @update:value="cb({ downIngoredList: $event })" />
    </div>
  </div>
</template>

<style></style>
