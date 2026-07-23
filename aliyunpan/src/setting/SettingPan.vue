<script setup lang="ts">
import { onMounted, ref, computed } from 'vue'
import useSettingStore from './settingstore'
import MySwitch from '../layout/MySwitch.vue'
import UserDAL from '../user/userdal'
import type { ITokenInfo } from '../user/userstore'
import { t } from '../i18n'

const settingStore = useSettingStore()
const cb = (val: any) => {
  settingStore.updateStore(val)
}

const userList = ref<ITokenInfo[]>([])

const refreshUserList = async () => {
  userList.value = await UserDAL.GetUserListFromDB()
}

onMounted(() => {
  refreshUserList().catch(() => {})
})

const tokenLabel = (token: ITokenInfo) => {
  const provider =
    token.tokenfrom === 'aliyun' ? t('settings.pan.provider.aliyun') :
    token.tokenfrom === 'cloud123' ? t('settings.pan.provider.cloud123') :
    token.tokenfrom === '115' ? t('settings.pan.provider.115') :
    token.tokenfrom === 'baidu' ? t('settings.pan.provider.baidu') :
    token.tokenfrom === 'pikpak' ? 'PikPak' :
    token.tokenfrom === 'dropbox' ? 'Dropbox' :
    token.tokenfrom === 'onedrive' ? 'OneDrive' :
    token.tokenfrom === 'box' ? 'Box' :
    t('settings.pan.provider.cloudDrive')
  const name = token.nick_name || token.user_name || token.user_id
  return `${provider} · ${name}`
}

const isMusicOn = (uid: string) => !(settingStore.uiLibraryAutoScanMusicDisabledUsers || []).includes(uid)
const isVideoOn = (uid: string) => !(settingStore.uiLibraryAutoScanVideoDisabledUsers || []).includes(uid)
const isBookOn = (uid: string) => !(settingStore.uiLibraryAutoScanBookDisabledUsers || []).includes(uid)

const toggleMusicForUser = (uid: string, on: boolean) => {
  const list = new Set(settingStore.uiLibraryAutoScanMusicDisabledUsers || [])
  if (on) list.delete(uid)
  else list.add(uid)
  cb({ uiLibraryAutoScanMusicDisabledUsers: Array.from(list) })
}

const toggleVideoForUser = (uid: string, on: boolean) => {
  const list = new Set(settingStore.uiLibraryAutoScanVideoDisabledUsers || [])
  if (on) list.delete(uid)
  else list.add(uid)
  cb({ uiLibraryAutoScanVideoDisabledUsers: Array.from(list) })
}

const toggleBookForUser = (uid: string, on: boolean) => {
  const list = new Set(settingStore.uiLibraryAutoScanBookDisabledUsers || [])
  if (on) list.delete(uid)
  else list.add(uid)
  cb({ uiLibraryAutoScanBookDisabledUsers: Array.from(list) })
}

const allMusicOff = () => {
  const ids = userList.value.map((t) => t.user_id).filter(Boolean)
  cb({ uiLibraryAutoScanMusicDisabledUsers: ids })
}
const allMusicOn = () => {
  cb({ uiLibraryAutoScanMusicDisabledUsers: [] })
}
const allVideoOff = () => {
  const ids = userList.value.map((t) => t.user_id).filter(Boolean)
  cb({ uiLibraryAutoScanVideoDisabledUsers: ids })
}
const allVideoOn = () => {
  cb({ uiLibraryAutoScanVideoDisabledUsers: [] })
}
const allBookOff = () => {
  const ids = userList.value.map((t) => t.user_id).filter(Boolean)
  cb({ uiLibraryAutoScanBookDisabledUsers: ids })
}
const allBookOn = () => {
  cb({ uiLibraryAutoScanBookDisabledUsers: [] })
}

const removeMusicFolder = (f: { user_id: string; drive_id: string; file_id: string }) => {
  const list = (settingStore.uiMusicAutoScanFolders || []).filter(
    (x: any) => !(x.user_id === f.user_id && x.drive_id === f.drive_id && x.file_id === f.file_id)
  )
  cb({ uiMusicAutoScanFolders: list })
}

const hasUsers = computed(() => userList.value.length > 0)
const showAccountList = computed(() =>
  (settingStore.uiLibraryAutoScanMusic || settingStore.uiLibraryAutoScanVideo || settingStore.uiLibraryAutoScanBook) && hasUsers.value
)
</script>

<template>
  <div class="settingcard">
    <div class="settings-panel-intro">
      <div class="settings-panel-kicker">Cloud Drive</div>
      <div class="settings-panel-copy">{{ t('settings.pan.intro') }}</div>
    </div>
    <div class="settinghead">{{ t('settings.pan.preferFolder') }}</div>
    <div class="settingrow">
      <a-select tabindex="-1" :style="{ width: '252px' }" :model-value="settingStore.uiShowPanRootFirst"
                :popup-container="'#SettingDiv'" @update:model-value="cb({ uiShowPanRootFirst: $event })">
        <a-option value="all">{{ t('settings.pan.all') }}</a-option>
        <a-option value="backup">{{ t('settings.pan.backupDrive') }}</a-option>
        <a-option value="resource">{{ t('settings.pan.resourceDrive') }}</a-option>
      </a-select>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.pan.showPathTitle') }}</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiShowPanPath" @update:value="cb({ uiShowPanPath: $event })">{{ t('settings.pan.showPathSwitch') }}</MySwitch>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.pan.showMediaTitle') }}</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiShowPanMedia" @update:value="cb({ uiShowPanMedia: $event })">{{ t('settings.pan.showMediaSwitch') }}</MySwitch>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.pan.folderPreviewTitle') }}</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiFolderPreviewEnabled" @update:value="cb({ uiFolderPreviewEnabled: $event })">{{ t('settings.pan.folderPreviewSwitch') }}</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultOn') }}
            <hr />
            {{ t('settings.pan.folderPreviewTip1') }}<br />
            {{ t('settings.pan.folderPreviewTip2') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div v-if="settingStore.uiFolderPreviewEnabled" class="settingrow">
      <span style="margin-right: 12px; color: var(--color-text-2)">{{ t('settings.pan.autoHideTime') }}</span>
      <a-select tabindex="-1" :style="{ width: '160px' }"
                :model-value="settingStore.uiFolderPreviewAutoHide"
                :popup-container="'#SettingDiv'"
                @update:model-value="cb({ uiFolderPreviewAutoHide: $event })">
        <a-option :value="0">{{ t('settings.pan.noAutoHide') }}</a-option>
        <a-option :value="3">{{ t('settings.pan.seconds3') }}</a-option>
        <a-option :value="6">{{ t('settings.pan.seconds6Recommended') }}</a-option>
        <a-option :value="10">{{ t('settings.pan.seconds10') }}</a-option>
        <a-option :value="20">{{ t('settings.pan.seconds20') }}</a-option>
      </a-select>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.pan.autoHideTip1') }}<br />
            {{ t('settings.pan.autoHideTip2') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.pan.folderSizeTitle') }}</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiFolderSize" @update:value="cb({ uiFolderSize: $event })">{{ t('settings.pan.folderSizeSwitch') }}</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultOn') }}
            <hr />
            {{ t('settings.pan.folderSizeTip1') }}<br />
            {{ t('settings.pan.folderSizeTip2') }}
            <div class="hrspace"></div>
            <span class="oporg">{{ t('settings.security.note') }}</span>{{ t('settings.pan.folderSizeNote') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.pan.libraryAutoScanTitle') }}</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiLibraryAutoScanMusic" @update:value="cb({ uiLibraryAutoScanMusic: $event })">{{ t('settings.pan.musicAutoScanSwitch') }}</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultOff') }}
            <hr />
            {{ t('settings.pan.musicAutoScanTip1') }}<br />
            {{ t('settings.pan.musicAutoScanTip2') }}<br />
            {{ t('settings.pan.firstLoginConsent') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiLibraryAutoScanVideo" @update:value="cb({ uiLibraryAutoScanVideo: $event })">{{ t('settings.pan.videoAutoScanSwitch') }}</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultOff') }}
            <hr />
            {{ t('settings.pan.videoAutoScanTip1') }}<br />
            {{ t('settings.pan.videoAutoScanTip2') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiLibraryAutoScanBook" @update:value="cb({ uiLibraryAutoScanBook: $event })">{{ t('settings.pan.bookAutoScanSwitch') }}</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultOff') }}
            <hr />
            {{ t('settings.pan.bookAutoScanTip1') }}<br />
            {{ t('settings.pan.firstLoginConsent') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div v-if="settingStore.uiLibraryAutoScanMusic || settingStore.uiLibraryAutoScanVideo || settingStore.uiLibraryAutoScanBook" class="settingrow">
      <MySwitch :value="settingStore.uiLibraryIncrementalScan" @update:value="cb({ uiLibraryIncrementalScan: $event })">{{ t('settings.pan.incrementalScanSwitch') }}</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultOn') }}
            <hr />
            {{ t('settings.pan.incrementalScanTipOn') }}<br />
            {{ t('settings.pan.incrementalScanTipOff') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div v-if="(settingStore.uiLibraryAutoScanMusic || settingStore.uiLibraryAutoScanVideo || settingStore.uiLibraryAutoScanBook) && settingStore.uiLibraryIncrementalScan" class="settingrow">
      <span style="margin-right: 12px; color: var(--color-text-2)">{{ t('settings.pan.scanInterval') }}</span>
      <a-select tabindex="-1" :style="{ width: '180px' }"
                :model-value="settingStore.uiLibraryScanIntervalHours"
                :popup-container="'#SettingDiv'"
                @update:model-value="cb({ uiLibraryScanIntervalHours: $event })">
        <a-option :value="1">{{ t('settings.pan.hour1') }}</a-option>
        <a-option :value="6">{{ t('settings.pan.hour6') }}</a-option>
        <a-option :value="12">{{ t('settings.pan.hour12') }}</a-option>
        <a-option :value="24">{{ t('settings.pan.hour24Recommended') }}</a-option>
        <a-option :value="72">{{ t('settings.pan.day3') }}</a-option>
        <a-option :value="168">{{ t('settings.pan.day7') }}</a-option>
      </a-select>
    </div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiLibraryFollowManualScans" @update:value="cb({ uiLibraryFollowManualScans: $event })">{{ t('settings.pan.followManualScansSwitch') }}</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.defaultOn') }}
            <hr />
            {{ t('settings.pan.followManualScansTip1') }}<br />
            {{ t('settings.pan.followManualScansTip2') }}<br />
            {{ t('settings.pan.followManualScansTip3') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div v-if="settingStore.uiLibraryFollowManualScans && (settingStore.uiMusicAutoScanFolders || []).length" class="settingrow library-scan-account-row">
      <div class="library-scan-account-head">
        <span style="color: var(--color-text-2); font-weight: 600">{{ t('settings.pan.musicManualScanFolders', { count: settingStore.uiMusicAutoScanFolders.length }) }}</span>
        <a-popconfirm :content="t('settings.pan.clearAudioAutoScanFoldersConfirm')" @ok="cb({ uiMusicAutoScanFolders: [] })">
          <a-button type="text" size="mini" status="warning">{{ t('settings.pan.clearAll') }}</a-button>
        </a-popconfirm>
      </div>
      <div class="library-scan-account-list">
        <div v-for="f in settingStore.uiMusicAutoScanFolders" :key="`${f.user_id}|${f.drive_id}|${f.file_id}`" class="library-scan-account-item">
          <div class="library-scan-account-name" :title="f.path || f.name">
            <span style="color: var(--color-text-3); margin-right: 6px">{{ f.path || '/' }}</span>
            {{ f.name || f.file_id }}
          </div>
          <a-button type="text" size="mini" status="danger" @click="removeMusicFolder(f)">{{ t('settings.pan.remove') }}</a-button>
        </div>
      </div>
    </div>
    <div v-if="showAccountList" class="settingrow library-scan-account-row">
      <div class="library-scan-account-head">
        <span style="color: var(--color-text-2); font-weight: 600">{{ t('settings.pan.scanAccounts') }}</span>
        <div class="library-scan-account-actions">
          <a-button v-if="settingStore.uiLibraryAutoScanMusic" type="text" size="mini" @click="allMusicOn">{{ t('settings.pan.enableAllMusic') }}</a-button>
          <a-button v-if="settingStore.uiLibraryAutoScanMusic" type="text" size="mini" status="warning" @click="allMusicOff">{{ t('settings.pan.disableAllMusic') }}</a-button>
          <a-button v-if="settingStore.uiLibraryAutoScanVideo" type="text" size="mini" @click="allVideoOn">{{ t('settings.pan.enableAllVideo') }}</a-button>
          <a-button v-if="settingStore.uiLibraryAutoScanVideo" type="text" size="mini" status="warning" @click="allVideoOff">{{ t('settings.pan.disableAllVideo') }}</a-button>
          <a-button v-if="settingStore.uiLibraryAutoScanBook" type="text" size="mini" @click="allBookOn">{{ t('settings.pan.enableAllBooks') }}</a-button>
          <a-button v-if="settingStore.uiLibraryAutoScanBook" type="text" size="mini" status="warning" @click="allBookOff">{{ t('settings.pan.disableAllBooks') }}</a-button>
        </div>
      </div>
      <div class="library-scan-account-list">
        <div v-for="token in userList" :key="token.user_id" class="library-scan-account-item">
          <div class="library-scan-account-name" :title="tokenLabel(token)">{{ tokenLabel(token) }}</div>
          <div class="library-scan-account-toggles">
            <span v-if="settingStore.uiLibraryAutoScanMusic" class="library-scan-toggle">
              <span class="library-scan-toggle-label">{{ t('settings.pan.music') }}</span>
              <MySwitch :value="isMusicOn(token.user_id)" @update:value="toggleMusicForUser(token.user_id, $event)">&nbsp;</MySwitch>
            </span>
            <span v-if="settingStore.uiLibraryAutoScanVideo" class="library-scan-toggle">
              <span class="library-scan-toggle-label">{{ t('settings.pan.video') }}</span>
              <MySwitch :value="isVideoOn(token.user_id)" @update:value="toggleVideoForUser(token.user_id, $event)">&nbsp;</MySwitch>
            </span>
            <span v-if="settingStore.uiLibraryAutoScanBook" class="library-scan-toggle">
              <span class="library-scan-toggle-label">{{ t('settings.pan.books') }}</span>
              <MySwitch :value="isBookOn(token.user_id)" @update:value="toggleBookForUser(token.user_id, $event)">&nbsp;</MySwitch>
            </span>
          </div>
        </div>
      </div>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.pan.independentFolderSort') }}</div>
    <div class="settingrow">
      <a-select tabindex="-1" :style="{ width: '252px' }" :model-value="settingStore.uiFileOrderDuli" :popup-container="'#SettingDiv'" @update:model-value="cb({ uiFileOrderDuli: $event })">
        <a-option value="null">
          {{ t('settings.pan.noIndependentSort') }}
          <template #suffix>{{ t('settings.pan.recommended') }}</template>
        </a-option>
        <a-option value="name asc">{{ t('settings.pan.sortNameAsc') }}</a-option>
        <a-option value="name desc">{{ t('settings.pan.sortNameDesc') }}</a-option>
        <a-option value="updated_at asc">{{ t('settings.pan.sortTimeAsc') }}</a-option>
        <a-option value="updated_at desc">{{ t('settings.pan.sortTimeDesc') }}</a-option>
        <a-option value="size asc">{{ t('settings.pan.sortSizeAsc') }}</a-option>
        <a-option value="size desc">{{ t('settings.pan.sortSizeDesc') }}</a-option>
      </a-select>
    </div>
  </div>
  <div class="settingcard">
    <div class="settinghead">{{ t('settings.pan.dateFolderTemplate') }}</div>
    <div class="settingrow">
      <a-input tabindex="-1" :style="{ width: '257px' }" placeholder="yyyy-MM-dd HH-mm-ss" allow-clear :model-value="settingStore.uiTimeFolderFormate" @update:model-value="cb({ uiTimeFolderFormate: $event })" />
      <a-input-number tabindex="-1" :style="{ width: '100px', marginLeft: '16px', marginTop: '-1px' }" :min="1" :model-value="settingStore.uiTimeFolderIndex" @update:model-value="cb({ uiTimeFolderIndex: $event })" />

      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div style="min-width: 400px">
            {{ t('settings.pan.defaultDateTemplate') }} <span class="opred">(2021-08-08 12-30-00)</span>
            <hr />
            {{ t('settings.pan.dateTemplateIntro') }}
            <br />
            {{ t('settings.pan.dateTemplateTokens') }}
            <div class="hrspace"></div>
            {{ t('settings.pan.dateTemplateIndexTip') }}
            <br />
            {{ t('settings.pan.dateTemplateHashTip') }}
            <div class="hrspace"></div>
            {{ t('settings.pan.example') }}<span class="oporg">{{ t('settings.pan.createdAtExampleTemplate') }}</span> --&gt;
            <span class="opblue">{{ t('settings.pan.createdAtExampleOutput') }}</span>
            <br />
            {{ t('settings.pan.example') }}<span class="oporg">{{ t('settings.pan.albumExampleTemplate') }}</span> --&gt;
            <span class="opblue">{{ t('settings.pan.albumExampleOutput') }}</span>
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.pan.shareDefaultsTitle') }}</div>
    <div class="settingrow flex">
      <a-radio-group type="button" tabindex="-1" :model-value="settingStore.uiShareDays" @update:model-value="cb({ uiShareDays: $event })">
        <a-radio tabindex="-1" value="always">{{ t('settings.pan.forever') }}</a-radio>
        <a-radio tabindex="-1" value="week">{{ t('settings.pan.oneWeek') }}</a-radio>
        <a-radio tabindex="-1" value="month">{{ t('settings.pan.oneMonth') }}</a-radio>
      </a-radio-group>

      <div style="margin-right: 8px"></div>

      <a-radio-group type="button" tabindex="-1" :model-value="settingStore.uiSharePassword" @update:model-value="cb({ uiSharePassword: $event })">
        <a-radio tabindex="-1" value="random">{{ t('settings.pan.random') }}</a-radio>
        <a-radio tabindex="-1" value="last">{{ t('settings.pan.last') }}</a-radio>
        <a-radio tabindex="-1" value="nopassword">{{ t('settings.pan.noPassword') }}</a-radio>
      </a-radio-group>

      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.pan.defaultForeverRandom') }}
            <hr />
            {{ t('settings.pan.foreverTip') }}
            <br />
            {{ t('settings.pan.weekTip') }}
            <br />
            {{ t('settings.pan.monthTip') }}
            <br />
            <div class="hrspace"></div>
            {{ t('settings.pan.randomTip') }}
            <br />
            {{ t('settings.pan.lastTip') }}
            <br />
            {{ t('settings.pan.noPasswordTip') }}
            <br />
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.pan.copyShareTemplate') }}</div>
    <div class="settingrow">
      <a-input tabindex="-1" :style="{ width: '257px' }" :placeholder="t('settings.pan.shareTemplatePlaceholder')" allow-clear :model-value="settingStore.uiShareFormate" @update:model-value="cb({ uiShareFormate: $event })" />

      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div style="min-width: 400px">
            {{ t('settings.pan.shareTemplateDefault') }} <br />
            {{ t('settings.pan.testShare') }}
            <hr />
            {{ t('settings.pan.shareTemplateIntro') }}
            <br />
            {{ t('settings.pan.shareTemplateTokens') }}

            <div class="hrspace"></div>
            {{ t('settings.pan.example') }}<span class="oporg">URL#PWD#NAME</span> --&gt; <br />
            <span class="opblue">https://www.aliyundrive.com/s/jEmmmDkF#DNJI#{{ t('settings.pan.testShareTitle') }}</span>
            <br />
            {{ t('settings.pan.example') }}<span class="oporg">URL Code: PWD NAME</span> --&gt; <br />
            <span class="opblue">https://www.aliyundrive.com/s/jEmmmDkF Code: DNJI {{ t('settings.pan.testShareTitle') }}</span>
          </div>
        </template>
      </a-popover>
    </div>
  </div>
  <div class="settingcard">
    <div class="settinghead">
      {{ t('settings.pan.fileTagCustomNames') }}
      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            {{ t('settings.pan.fileTagTip1') }}<br />
            {{ t('settings.pan.fileTagTip2') }}<br />
            {{ t('settings.pan.fileTagTip3') }}<br />
            <div class="hrspace"></div>
            <span class="oporg">{{ t('settings.pan.lightUse') }}</span>{{ t('settings.pan.fileTagTip4') }}
            <br />
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingrow">
      <a-row class="grid-demo">
        <a-col v-for="item in settingStore.uiFileColorArray" :key="item.key" flex="210px">
          <span style="width: 82px; display: inline-block"><IconFont name="iconcheckbox-full" :style="{ color: item.key }" />{{ item.key }}</span>
          <a-input :style="{ width: '120px' }" allow-clear :model-value="item.title" @update:model-value="(val:string)=>settingStore.updateFileColor(item.key,val)"> </a-input>
        </a-col>
      </a-row>
    </div>
  </div>
</template>

<style scoped>
.settings-panel-intro {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 14px;
}

.settings-panel-kicker {
  display: inline-flex;
  align-self: flex-start;
  padding: 4px 10px;
  border-radius: 999px;
  background: rgba(88, 130, 255, 0.12);
  color: var(--color-primary-6);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.settings-panel-copy {
  max-width: 620px;
  color: var(--color-text-2);
  font-size: 14px;
  line-height: 1.7;
}

:global(html.dark) .settings-panel-kicker {
  background: rgba(120, 160, 255, 0.2);
  color: #dbe6ff;
}

.library-scan-account-row {
  flex-direction: column;
  align-items: stretch;
  gap: 8px;
}

.library-scan-account-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.library-scan-account-actions {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}

.library-scan-account-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  border: 1px solid var(--color-border-2);
  border-radius: 8px;
  padding: 8px 12px;
  max-height: 240px;
  overflow-y: auto;
}

.library-scan-account-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 4px 0;
}

.library-scan-account-item + .library-scan-account-item {
  border-top: 1px dashed var(--color-border-2);
}

.library-scan-account-name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--color-text-1);
  font-size: 13px;
}

.library-scan-account-toggles {
  display: flex;
  gap: 12px;
  flex-shrink: 0;
}

.library-scan-toggle {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.library-scan-toggle-label {
  font-size: 12px;
  color: var(--color-text-3);
}
</style>
