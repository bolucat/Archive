<script setup lang='ts'>
import useSettingStore from './settingstore'
import MySwitch from '../layout/MySwitch.vue'
import WebDavServer from '../module/webdav'
import { reactive, ref } from 'vue'
import message from '../utils/message'
import { IUser } from 'webdav-server/lib/index.v2'
import { Sleep } from '../utils/format'
import AppCache from '../utils/appcache'
import { getUserData, openExternal } from '../utils/electronhelper'
import path from 'path'
import { getIPAddress } from '../utils/proxyhelper'
import { t } from '../i18n'

const cb = (val: any) => {
  if (Object.hasOwn(val, 'webDavPort') && val.webDavPort !== settingStore.webDavPort) {
    WebDavServer.config({ port: val.webDavPort })
  }
  if (Object.hasOwn(val, 'webDavHost') && val.webDavHost !== settingStore.webDavHost) {
    WebDavServer.config({ port: val.webDavHost })
  }
  settingStore.updateStore(val)
}

const settingStore = useSettingStore()
const loading = ref(false)
const options = ref<IUser[]>([])
// 弹窗
const addVisible = ref(false)
const okLoading = ref(false)
const formRef = ref()
const selectUser = ref()
const form = reactive({
  webDavUsername: '',
  webDavPassword: '',
  webDavPath: '/',
  webDavRights: ['all']
})

const handleWebDav = async (newVal: any) => {
  if (!WebDavServer) {
    message.error(t('settings.webdav.initFailed'))
    cb({ webDavEnable: false })
    return false
  }
  try {
    let status: boolean = true
    if (newVal) {
      status = await WebDavServer.config({
        port: settingStore.webDavPort,
        hostname: settingStore.webDavHost,
        requireAuthentification: false
      }).start()
      if (status) {
        message.success(t('settings.webdav.started'))
        cb({ webDavEnable: true })
      } else {
        message.error(t('settings.webdav.startFailed'))
        cb({ webDavEnable: false })
      }
    } else {
      await WebDavServer.stop()
      message.success(t('settings.webdav.stopped'))
      cb({ webDavEnable: false })
    }
    await Sleep(200)
    return status
  } catch (error: any) {
    message.error(`【WebDav】:${error}`)
    return false
  }
}

const handleGetLocalIp = () => {
  if (settingStore.webDavHost.includes('127')) {
    let localIp = getIPAddress()
    cb({ webDavHost: localIp })
  } else {
    cb({ webDavHost: '127.0.0.1' })
  }
}

const handleGetUsers = (visible: boolean) => {
  if (visible) {
    loading.value = true
    setTimeout(async () => {
      options.value = await WebDavServer.getAllUser()
      loading.value = false
    }, 200)
  }
}

const handleChangeUser = async (value: any) => {
  if (value) {
    selectUser.value = await WebDavServer.getUser(value)
  }
}

const handleAddUser = () => {
  addVisible.value = true
  form.webDavUsername = ''
  form.webDavPassword = ''
  form.webDavPath = '/'
  form.webDavRights = ['all']
}
const handleModifyUser = () => {
  if (selectUser.value) {
    addVisible.value = true
    form.webDavUsername = selectUser.value.username
    form.webDavPassword = selectUser.value.password || ''
    form.webDavPath = selectUser.value.path || ''
    form.webDavRights = selectUser.value.rights
  } else {
    message.error(t('settings.webdav.noUserSelected'))
  }
}
const handleDelUser = () => {
  if (selectUser.value) {
    WebDavServer.delUser(selectUser.value.username)
  } else {
    message.error(t('settings.webdav.noUserSelected'))
  }
}
const handleAddOk = async () => {
  formRef.value.validate(async (data: any) => {
    if (data) return
    // 添加用户
    const success = await WebDavServer.setUser(form.webDavUsername, form.webDavPassword, form.webDavPath, form.webDavRights, false)
    if (success) {
      message.success(t('settings.webdav.addUserSuccess'))
    } else {
      message.error(t('settings.webdav.addUserFailed'))
    }
    addVisible.value = false
  })
}

const handleRightsOption = (value: any) => {
  if (value) {
    if (value.includes('all') || value.length >= 2) {
      form.webDavRights = ['all']
    }
  }
}
const handleAddCancel = () => {
  addVisible.value = false
  if (okLoading.value) okLoading.value = false
  formRef.value.resetFields()
}
const handleBeforeClose = () => {
  if (okLoading.value) okLoading.value = false
  formRef.value.resetFields()
}
const handleJumpPath = () => {
  const userData = getUserData()
  openExternal(path.join(userData, 'Cache'))
}
</script>

<template>
  <div class='settingcard'>
    <div class='settinghead'>{{ t('settings.webdav.settings') }}</div>
    <div class='settingrow'>
      <MySwitch v-model:value='settingStore.webDavEnable' :beforeChange='handleWebDav'>
        {{ t('settings.webdav.enable') }}
      </MySwitch>
    </div>
    <div class='settingrow'>
      <MySwitch :value="settingStore.webDavAutoEnable"
                @update:value="cb({ webDavAutoEnable: $event })">
        {{ t('settings.webdav.autoStart') }}
      </MySwitch>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>{{ t('settings.webdav.host') }}</div>
    <div class='settingrow'>
      <a-input-search tabindex="-1"
                      :disabled="settingStore.webDavEnable"
                      style="width: 320px;"
                      v-model.trim='settingStore.webDavHost'
                      button-text='ip'
                      allow-clear
                      search-button
                      @search="handleGetLocalIp"
                      :placeholder="t('settings.webdav.hostPlaceholder')" @update:model-value='cb({ webDavHost: $event })'>
        <template #prefix> http://</template>
        <template #suffix> /webdav</template>
      </a-input-search>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>{{ t('settings.webdav.port') }}</div>
    <div class='settingrow'>
      <a-input-number
        :disabled="settingStore.webDavEnable"
        tabindex='-1' :style="{ width: '320px' }"
        :placeholder="t('settings.webdav.defaultPort')"
        :model-value='settingStore.webDavPort'
        @update:model-value='cb({ webDavPort: $event })' />
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>{{ t('settings.webdav.cacheSeconds') }}</div>
    <div class='settingrow'>
      <a-input-number
        tabindex='-1' :style="{ width: '320px' }"
        hide-button :placeholder="t('settings.webdav.defaultCache')"
        :model-value='settingStore.webDavListCache'
        @update:model-value='cb({ webDavListCache: $event })' />
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>{{ t('settings.webdav.accessStrategy') }}</div>
    <div class='settingrow'>
      <a-select tabindex="-1" :style="{ width: '320px' }"
                :model-value="settingStore.webDavStrategy"
                :popup-container="'#SettingDiv'"
                @update:model-value="cb({ webDavStrategy: $event })">
        <a-option value='redirect'>{{ t('settings.webdav.redirect') }}</a-option>
        <a-option value='proxy'>{{ t('settings.webdav.localProxy') }}</a-option>
      </a-select>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>{{ t('settings.webdav.userList') }}</div>
    <div class='settingrow'>
      <a-select @popup-visible-change='handleGetUsers'
                @change='handleChangeUser'
                :field-names="{ key: 'uid', value: 'username', label: 'username'}"
                :virtual-list-props='{height:120}'
                :options='options'
                :style="{width:'320px'}"
                :placeholder="t('settings.webdav.selectUser')"
                :popup-container="'#SettingDiv'"
                :loading='loading' allow-clear
                :allow-search='{ retainInputValue: true }' scrollbar>
      </a-select>
    </div>
    <a-modal modal-class='modalclass' :footer='false'
             v-model:visible='addVisible' :title="t('settings.webdav.addUser')" unmountOnClose
             @cancel='handleAddCancel' @before-close='handleBeforeClose'>
      <a-space direction='vertical' size='large' :style="{width: '400px'}">
        <a-form ref='formRef' auto-label-width :model='form'>
          <a-form-item field='webDavUsername' :label="t('settings.webdav.username')" :rules="{ required: true, message: t('settings.webdav.usernameRequired') }">
            <a-input tabindex='-1'
                     v-model.trim='form.webDavUsername'
                     :placeholder="t('settings.webdav.usernamePlaceholder')"
                     allow-clear />
          </a-form-item>
          <a-form-item field='webDavPassword' :label="t('settings.webdav.password')"
                       :rules="[
                         { required: true, message: t('settings.webdav.passwordRequired') },
                         { minLength: 6, message: t('settings.webdav.passwordMin') }
                       ]">
            <a-input
              tabindex='-1'
              v-model.trim='form.webDavPassword'
              :placeholder="t('settings.webdav.passwordPlaceholder')"
              allow-clear />
          </a-form-item>
          <a-form-item field='webDavPath' :label="t('settings.webdav.mountPath')" :rules="{ required: true, message: t('settings.webdav.mountPathRequired') }">
            <a-input v-model.trim='form.webDavPath' :placeholder="t('settings.webdav.mountPathPlaceholder')" />
          </a-form-item>
          <a-form-item field='webDavRights' :label="t('settings.webdav.mountRights')" :rules="{ required: true, message: t('settings.webdav.mountRightsRequired') }">
            <a-select v-model='form.webDavRights'
                      @change='handleRightsOption'
                      multiple :max-tag-count='3'
                      :placeholder="t('settings.webdav.mountRightsPlaceholder')">
              <a-option value='all'>{{ t('settings.webdav.rightsAll') }}</a-option>
              <a-option value='canRead'>{{ t('settings.webdav.rightsRead') }}</a-option>
              <a-option value='canWrite'>{{ t('settings.webdav.rightsWrite') }}</a-option>
            </a-select>
          </a-form-item>
        </a-form>
      </a-space>
      <div class='modalfoot'>
        <div class='modalfoot-spacer'></div>
        <a-button v-if='!okLoading' type='outline' size='small' @click='handleAddCancel'>{{ t('common.cancel') }}</a-button>
        <a-button type='primary' size='small' :loading='okLoading' @click='handleAddOk'>{{ t('common.add') }}</a-button>
      </div>
    </a-modal>
    <div class='settingspace'></div>
    <div class='settingrow'>
      <a-button type='primary' status='normal' size='small' tabindex='-1' @click='handleAddUser'>{{ t('common.add') }}</a-button>
      <a-button type='primary' status='success' size='small' @click='handleModifyUser'>{{ t('common.edit') }}</a-button>
      <a-popconfirm :content="t('settings.webdav.confirmDeleteUser')" @ok='handleDelUser'>
        <a-button type='primary' status='danger' size='small'>{{ t('common.delete') }}</a-button>
      </a-popconfirm>
    </div>
    <template v-if="settingStore.webDavStrategy === 'proxy'">
      <div class='settingspace'></div>
      <div class="settinghead">
        {{ t('settings.webdav.cacheSize') }}
        <span class="opblue cache-size-badge">( {{ settingStore.debugCacheSize }} )</span>
      </div>
      <div class="settingrow">
        <a-button type='outline' size='small' @click='handleJumpPath'>{{ t('settings.debug.openLocation') }}</a-button>
        <a-popconfirm :content="t('settings.debug.confirmClearCache')" @ok="AppCache.aClearCache()">
          <a-button type="outline" size="small" status="danger">{{ t('settings.debug.clearCache') }}</a-button>
        </a-popconfirm>
      </div>
    </template>
  </div>
</template>

<style scoped>
.modalfoot-spacer { flex: 1; }
.cache-size-badge { margin-left: 12px; padding: 0 12px; }
</style>
