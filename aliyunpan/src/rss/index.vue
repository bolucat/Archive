<script setup lang="ts">
import { useAppStore } from '../store'
import usePanTreeStore from '../pan/pantreestore'
import { computed, watch } from 'vue'
import { isAliyunUser } from '../aliapi/utils'
import RssScanClean from './rssscanclean/RssScanClean.vue'
import AppSame from './appsame/AppSame.vue'
import RssXiMa from './rssxima/RssXiMa.vue'
import RssScanSame from './rssscansame/RssScanSame.vue'
import RssScanPunish from './rssscanpunish/RssScanPunish.vue'
import RssScanEnmpty from './rssscanenmpty/RssScanEnmpty.vue'
import RssJiaMi from './rssjiami/RssJiaMi.vue'
import RssDriveCopy from './rssdrivecopy/RssDriveCopy.vue'
import RssEmptyDirs from './drivetools/RssEmptyDirs.vue'
import { t } from '../i18n'

const appStore = useAppStore()
const panTreeStore = usePanTreeStore()
const isAliyunAccount = computed(() => isAliyunUser(panTreeStore.user_id || ''))
const aliyunOnlyMenus = new Set(['AppSame', 'RssScanPunish', 'RssScanEnmpty', 'RssDriveCopy'])
const removedMenus = new Set(['RssDriveTools', 'RssMoveOrganize', 'RssMediaOrganize', 'RssRename', 'RssUserCopy'])

watch(
  () => [isAliyunAccount.value, appStore.GetAppTabMenu],
  ([isAliyun]) => {
    if (removedMenus.has(appStore.GetAppTabMenu)) {
      appStore.toggleTabMenu('rss', 'RssEmptyDirs')
      return
    }
    if (!isAliyun && aliyunOnlyMenus.has(appStore.GetAppTabMenu)) {
      appStore.toggleTabMenu('rss', 'RssXiMa')
    }
  },
  { immediate: true }
)
</script>

<template>
  <a-layout style="height: 100%">
    <a-layout-sider hide-trigger :width="218" class="xbyleft rss-sider single-boundary-sidebar">
      <div class="headdesc">{{ t('plugins.title') }}</div>
      <a-menu :style="{ width: '100%' }" class="xbyleftmenu rss-leftmenu single-boundary-sidebar-menu"
              :selected-keys="[appStore.GetAppTabMenu]"
              @update:selected-keys="appStore.toggleTabMenu('rss', $event[0])">
        <a-menu-item key="RssXiMa">
          <template #icon><IconFont name="iconcameraadd" /></template>
          {{ t('plugins.washCode') }}
        </a-menu-item>
        <a-menu-item key="RssJiaMi">
          <template #icon><IconFont name="iconsafebox" /></template>
          {{ t('plugins.encrypt') }}
        </a-menu-item>
        <a-menu-item key="RssEmptyDirs">
          <template #icon><IconFont name="iconempty" /></template>
          {{ t('plugins.emptyDirs') }}
        </a-menu-item>
        <a-menu-item v-if="isAliyunAccount" key="AppSame">
          <template #icon><IconFont name="iconcopy" /></template>
          {{ t('plugins.duplicates') }}
        </a-menu-item>
        <a-menu-item key="RssScanClean">
          <template #icon><IconFont name="iconclear" /></template>
          {{ t('plugins.largeFiles') }}
        </a-menu-item>
        <a-menu-item key="RssScanSame">
          <template #icon><IconFont name="iconcopy" /></template>
          {{ t('plugins.scanDuplicates') }}
        </a-menu-item>
        <a-menu-item v-if="isAliyunAccount" key="RssScanPunish">
          <template #icon><IconFont name="iconweixiang" /></template>
          {{ t('plugins.violations') }}
        </a-menu-item>
        <a-menu-item v-if="isAliyunAccount" key="RssScanEnmpty">
          <template #icon><IconFont name="iconempty" /></template>
          {{ t('plugins.emptyFiles') }}
        </a-menu-item>
        <a-menu-item v-if="isAliyunAccount" key="RssDriveCopy">
          <template #icon><IconFont name="iconchuanshu2" /></template>
          {{ t('plugins.albumCopy') }}
        </a-menu-item>
      </a-menu>
    </a-layout-sider>
    <a-layout-content class="rss-content-panel">
      <a-tabs type="text" :direction="'horizontal'" class="hidetabs" :justify="true" :active-key="appStore.GetAppTabMenu">
        <a-tab-pane key="RssXiMa" title="1"><RssXiMa /></a-tab-pane>
        <a-tab-pane key="RssJiaMi" title="3"><RssJiaMi /></a-tab-pane>
        <a-tab-pane key="RssEmptyDirs" title="7"><RssEmptyDirs /></a-tab-pane>
        <a-tab-pane v-if="isAliyunAccount" key="AppSame" title="4"><AppSame /></a-tab-pane>
        <a-tab-pane key="RssScanClean" title="6"><RssScanClean /></a-tab-pane>
        <a-tab-pane key="RssScanSame" title="7"><RssScanSame /></a-tab-pane>
        <a-tab-pane v-if="isAliyunAccount" key="RssScanPunish" title="8"><RssScanPunish /></a-tab-pane>
        <a-tab-pane v-if="isAliyunAccount" key="RssScanEnmpty" title="9"><RssScanEnmpty /></a-tab-pane>
        <a-tab-pane v-if="isAliyunAccount" key="RssDriveCopy" title="10"><RssDriveCopy /></a-tab-pane>
      </a-tabs>
    </a-layout-content>
  </a-layout>
</template>

<style>
.iconnode-tree1,
.iconshuzhuangtu {
  opacity: 0.8;
}

.rss-sider {
  min-width: 218px;
}

.rss-leftmenu .arco-menu-item {
  padding-right: 14px !important;
}

.rss-content-panel {
  min-width: 0;
  height: 100%;
  padding: 18px 18px 18px 14px;
  overflow: hidden;
  background: transparent !important;
}

.rss-content-panel .hidetabs {
  height: 100%;
}

.rss-content-panel .rightbg {
  height: 100%;
  margin: 0;
  overflow-y: auto !important;
  overflow-x: hidden !important;
}

body:not([arco-theme='dark']) #xbybody .rss-content-panel > .hidetabs {
  background: var(--color-bg-1) !important;
  border-color: var(--color-border-2) !important;
  box-shadow: none !important;
  backdrop-filter: none !important;
}

body:not([arco-theme='dark']) #xbybody .rss-content-panel .rightbg {
  background: var(--color-bg-1) !important;
  border-color: var(--color-border-2) !important;
  box-shadow: none !important;
  backdrop-filter: none !important;
}
</style>
