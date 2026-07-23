<script setup lang='ts'>
import useSettingStore from './settingstore'
import MySwitch from '../layout/MySwitch.vue'
import { AriaGlobalSpeed } from '../utils/aria2c'
import { t } from '../i18n'

const settingStore = useSettingStore()

const cb = async (val: any) => {
  await settingStore.updateStore(val)
  // 限速实时生效
  if (Object.hasOwn(val, 'downGlobalSpeed') || Object.hasOwn(val, 'downGlobalSpeedM')) {
    await AriaGlobalSpeed()
  }
}

const handleSelectDownSavePath = () => {
  if (window.WebShowOpenDialogSync) {
    window.WebShowOpenDialogSync(
      {
        title: t('settings.download.selectSaveFolder'),
        buttonLabel: t('media.selectFolder'),
        properties: ['openDirectory', 'createDirectory'],
        defaultPath: settingStore.downSavePath
      },
      (result: string[] | undefined) => {
        if (result && result[0]) {
          settingStore.updateStore({ downSavePath: result[0] })
        }
      }
    )
  }
}
</script>

<template>
<!--  <div class='settingcard'>-->
<!--    <div class='settinghead'>{{ t('settings.download.engine') }}</div>-->
<!--    <div class='settingrow'>-->
<!--      <span>{{ t('settings.download.useAria2') }}</span>-->
<!--      <a-popover position='bottom'>-->
<!--        <IconFont name="iconbulb" />-->
<!--        <template #content>-->
<!--          <div>-->
<!--            <span class='opred'>{{ t('settings.download.engineCurrent') }}</span>-->
<!--            <hr />-->
<!--            {{ t('settings.download.engineTip') }}-->
<!--          </div>-->
<!--        </template>-->
<!--      </a-popover>-->
<!--    </div>-->
<!--  </div>-->

  <div class='settingcard'>
    <div class='settinghead'>{{ t('settings.download.savePath') }}</div>
    <div class='settingrow'>
      <a-input-search tabindex='-1' class='down-path-input' :readonly='true' :button-text="t('settings.download.change')" search-button
                      :model-value='settingStore.downSavePath' @search='handleSelectDownSavePath' />
    </div>
    <div class='settingrow'>
      <MySwitch :value='settingStore.downSavePathDefault' @update:value='cb({ downSavePathDefault: $event })'> {{ t('settings.download.newTask') }}
        {{ t('settings.download.defaultUsePath') }}
      </MySwitch>
      <a-popover position='bottom'>
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            <span class='opred'>{{ t('settings.defaultOn') }}</span>
            <hr />
            {{ t('settings.download.defaultPathTip') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class='settingrow'>
      <MySwitch :value='settingStore.downSavePathFull' @update:value='cb({ downSavePathFull: $event })'> {{ t('settings.download.newTask') }}
        {{ t('settings.download.fullPath') }}
      </MySwitch>
      <a-popover position='bottom'>
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            <span class='opred'>{{ t('settings.defaultOn') }}</span>
            <hr />
            {{ t('settings.download.fullPathTip') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class='settingrow'>
      <MySwitch :value='settingStore.downSaveBreakWeiGui' @update:value='cb({ downSaveBreakWeiGui: $event })'> {{ t('settings.download.newTask') }}
        {{ t('settings.download.skipRestricted') }}
      </MySwitch>
      <a-popover position='bottom'>
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            <span class='opred'>{{ t('settings.defaultOn') }}</span>
            <hr />
            {{ t('settings.download.skipRestrictedTip') }}
          </div>
        </template>
      </a-popover>
    </div>
  </div>

  <div class='settingcard'>
    <div class='settinghead'>{{ t('settings.download.maxParallel') }}</div>
    <div class='settingrow'>
      <a-input-number
        tabindex='-1' :style="{ width: '252px' }"
        mode='button'
        :min='1' :max='5' :step='1'
        :model-value='settingStore.downFileMax'
        @update:model-value='cb({ downFileMax: $event })'>
        <template #prefix> {{ t('settings.download.simultaneous') }}</template>
        <template #suffix> {{ t('settings.download.files') }}</template>
      </a-input-number>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>{{ t('settings.download.threadsPerFile') }}</div>
    <div class='settingrow'>
      <a-select tabindex='-1' :style="{ width: '252px' }" :model-value='settingStore.downThreadMax'
                :popup-container="'#SettingDiv'" @update:model-value='cb({ downThreadMax: $event })'>
        <a-option :value='1'>{{ t('settings.download.threadOptionPrefix') }} 1 {{ t('settings.download.threadOptionSuffix') }}</a-option>
        <a-option :value='2'>{{ t('settings.download.threadOptionPrefix') }} 2 {{ t('settings.download.threadOptionSuffix') }}</a-option>
        <a-option :value='4'>{{ t('settings.download.threadOptionPrefix') }} 4 {{ t('settings.download.threadOptionSuffix') }}</a-option>
        <a-option :value='8'>{{ t('settings.download.threadOptionPrefix') }} 8 {{ t('settings.download.threadOptionSuffix') }}</a-option>
        <a-option :value='16'>{{ t('settings.download.threadOptionPrefix') }} 16 {{ t('settings.download.threadOptionSuffix') }}</a-option>
        <a-option :value='24'>{{ t('settings.download.threadOptionPrefix') }} 24 {{ t('settings.download.threadOptionSuffix') }}</a-option>
        <a-option :value='32'>{{ t('settings.download.threadOptionPrefix') }} 32 {{ t('settings.download.threadOptionSuffix') }}</a-option>
      </a-select>
      <a-popover position='right'>
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            <span class='opred'>{{ t('settings.download.threadDefault') }}</span>
            <hr />
            {{ t('settings.download.threadTip') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>{{ t('settings.download.maxConnPerServer') }} <span style="font-weight:400;color:var(--color-text-3);font-size:12px">Max Connection Per Server</span></div>
    <div class='settingrow'>
      <a-input-number
        tabindex='-1' :style="{ width: '252px' }"
        mode='button'
        :min='1' :max='64' :step='1'
        :model-value='settingStore.ariaMaxConnectionPerServer'
        @update:model-value='cb({ ariaMaxConnectionPerServer: $event })'>
        <template #prefix> {{ t('settings.download.perServer') }}</template>
        <template #suffix> {{ t('settings.download.connections') }}</template>
      </a-input-number>
      <a-popover position='right'>
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            <span class='opred'>16</span>
            <hr />
            {{ t('settings.download.maxConnTip') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>{{ t('settings.download.globalSpeed') }}</div>
    <div class='settingrow'>
      <a-input-number
        tabindex='-1' :style="{ width: '128px' }"
        mode='button' :min='0'
        :max="settingStore.downGlobalSpeedM == 'MB' ? 100 : 999"
        :step="settingStore.downGlobalSpeedM == 'MB' ? 4 : 40"
        :model-value='settingStore.downGlobalSpeed'
        @update:model-value='cb({ downGlobalSpeed: $event })'>
      </a-input-number>
      <div class='down-divider'></div>
      <a-radio-group type='button'
                     tabindex='-1' :model-value='settingStore.downGlobalSpeedM'
                     @update:model-value='cb({ downGlobalSpeedM: $event, downGlobalSpeed: 0 })'>
        <a-radio tabindex='-1' value='MB'>MB/s</a-radio>
        <a-radio tabindex='-1' value='KB'>KB/s</a-radio>
      </a-radio-group>
      <a-popover position='bottom'>
        <IconFont name="iconbulb" />
        <template #content>
          <div :style="{ width: '360px' }">
            <span class='opred'>{{ t('settings.download.speedDefault') }}</span>
            <hr />
            <span class='opred'>{{ t('settings.download.speedTip100') }}</span><br />
            <span class='opred'>{{ t('settings.download.speedTipKb') }}</span><br />
            {{ t('settings.download.speedTip') }}
          </div>
        </template>
      </a-popover>
    </div>
  </div>
</template>

<style>
.down-path-input { max-width: 420px; }
.down-divider { height: 32px; border-left: 1px solid var(--color-neutral-3); }
</style>
