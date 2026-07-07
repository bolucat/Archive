<script setup lang='ts'>
import { computed, onMounted, ref } from 'vue'
import useSettingStore from './settingstore'
import MySwitch from '../layout/MySwitch.vue'
import LimitReachedModal from './LimitReachedModal.vue'
import { createClient } from '@supabase/supabase-js'
import Config from '../config'
import { openExternal } from '../utils/electronhelper'
import { Github, Chrome, Mail, Loader2, LogOut } from 'lucide-vue-next'
import ServerHttp from '../aliapi/server'
import os from 'os'
import { getAppNewPath, getResourcesPath } from '../utils/electronhelper'
import { existsSync, readFileSync } from 'fs'
import { getPkgVersion } from '../utils/utils'
import { modalUpdateLog } from '../utils/modal'
import fs from 'node:fs'
import message from '../utils/message'
import { Sleep } from '../utils/format'

const platform = window.platform
const settingStore = useSettingStore()

const isPro = ref(false)
const isLoggedIn = ref(false)
const accountEmail = ref('')
try {
  isPro.value = localStorage.getItem('app_user_pro') === '1'
  isLoggedIn.value = localStorage.getItem('app_user_authed') === '1'
  accountEmail.value = localStorage.getItem('app_user_email') || ''
} catch {}
const showUpgradeModal = ref(false)

onMounted(() => {
  setupAuthCallback()
  if (isLoggedIn.value) syncProStatus()
  // Listen for payment success callback from website
  if (window.Electron?.ipcRenderer) {
    window.Electron.ipcRenderer.on('payment-callback', () => {
      syncProStatus()
      message.success('支付完成，正在同步 Pro 状态…')
    })
  }
  if (localStorage.getItem('boxplayer_show_pricing') === '1') {
    localStorage.removeItem('boxplayer_show_pricing')
    if (!isPro.value) showUpgradeModal.value = true
  }
})

function handleLogout() {
  localStorage.removeItem('app_user_email')
  localStorage.removeItem('app_user_authed')
  isLoggedIn.value = false
  accountEmail.value = ''
  supabase?.auth.signOut().catch(() => {})
  message.success('已退出登录')
}

const loading = ref(false)
const showEmail = ref(false)
const codeSent = ref(false)
const emailCode = ref('')
const authEmail = ref('')
const upgrading = ref(false)

const CALLBACK_URL = 'boxplayer-auth://callback'

const supabase = Config.SUPABASE_URL && Config.SUPABASE_ANON_KEY
  ? createClient(Config.SUPABASE_URL, Config.SUPABASE_ANON_KEY) : null

function saveLogin(email: string) {
  localStorage.setItem('app_user_email', email)
  localStorage.setItem('app_user_authed', '1')
  accountEmail.value = email
  isLoggedIn.value = true
  syncProStatus()
}

async function syncProStatus() {
  if (!isLoggedIn.value || !supabase) return
  try {
    const { data: { user } } = await supabase.auth.getUser()
    if (!user) return
    const { data: subs } = await supabase.from('user_subscriptions').select('status').eq('user_id', user.id).maybeSingle()
    if (subs?.status === 'active' || subs?.status === 'trialing') {
      localStorage.setItem('app_user_pro', '1')
      isPro.value = true
    }
  } catch {}
}

async function handleOAuth(provider: 'github' | 'google') {
  if (!supabase) { message.error('未配置 Supabase'); return }
  loading.value = true
  try {
    const { data, error } = await supabase.auth.signInWithOAuth({
      provider, options: { redirectTo: CALLBACK_URL, skipBrowserRedirect: true },
    })
    if (error) message.error(error.message)
    else if (data.url) openExternal(data.url)
  } finally { loading.value = false }
}

async function handleEmailSend() {
  const email = authEmail.value.trim()
  if (!email?.includes('@')) { message.warning('请输入有效邮箱'); return }
  if (!supabase) { message.error('未配置 Supabase'); return }
  loading.value = true
  try {
    const { error } = await supabase.auth.signInWithOtp({ email, options: { shouldCreateUser: true } })
    if (error) message.error(error.message)
    else { codeSent.value = true; message.success('验证码已发送') }
  } finally { loading.value = false }
}

async function handleEmailVerify() {
  const email = authEmail.value.trim()
  const token = emailCode.value.trim()
  if (!token) { message.warning('请输入验证码'); return }
  if (!supabase) return
  loading.value = true
  try {
    const { data, error } = await supabase.auth.verifyOtp({ email, token, type: 'email' })
    if (error) message.error(error.message)
    else if (data.user) { saveLogin(data.user.email || email); message.success('登录成功') }
  } finally { loading.value = false }
}

async function handleUpgrade() {
  openExternal('https://xbysite.pages.dev/#pricing')
  } catch (e: any) { message.error(e?.message || '网络请求失败') }
  finally { upgrading.value = false }
}

function setupAuthCallback() {
  if (!window.Electron?.ipcRenderer) return
  const handler = async (_e: any, params: { access_token?: string; refresh_token?: string }) => {
    if (!params.access_token || !supabase) return
    const { data, error } = await supabase.auth.setSession({ access_token: params.access_token, refresh_token: params.refresh_token || '' })
    if (!error && data.user) { saveLogin(data.user.email || ''); message.success('登录成功') }
  }
  window.Electron.ipcRenderer.on('auth-callback', handler)
}

function setupPaymentCallback() {
  if (!window.Electron?.ipcRenderer) return
  const handler = async (_e: any, params: { checkout_id?: string }) => {
    if (params.checkout_id && Config.CREEM_API_KEY) {
      try {
        const apiBase = Config.CREEM_API_KEY.startsWith('creem_test_') ? 'https://test-api.creem.io' : 'https://api.creem.io'
        const resp = await fetch(`${apiBase}/v1/checkouts/${params.checkout_id}`, { headers: { 'x-api-key': Config.CREEM_API_KEY } })
        const data = await resp.json()
        if (data?.status === 'completed') { localStorage.setItem('app_user_pro', '1'); isPro.value = true; message.success('Pro 已激活！') }
      } catch {}
    }
  }
  window.Electron.ipcRenderer.on('payment-callback', handler)
}

const cb = (val: any) => {
  settingStore.updateStore(val)
}

const getAppVersion = computed(() => {
  const pkgVersion = getPkgVersion()
  if (os.platform() === 'linux') {
    return pkgVersion
  }
  return pkgVersion
  // let appVersion = ''
  // const localVersion = getResourcesPath('localVersion')
  // if (localVersion && existsSync(localVersion)) {
  //   appVersion = readFileSync(localVersion, 'utf-8')
  // } else {
  //   appVersion = pkgVersion
  // }
  // return appVersion
})

const verLoading = ref(false)
const handleCheckVer = () => {
  verLoading.value = true
  setTimeout(() => {
    ServerHttp.CheckUpgrade()
    verLoading.value = false
  }, 200)
}
const handleUpdateLog = () => {
  modalUpdateLog()
}

const handleImportAsar = () => {
  window.WebShowOpenDialogSync({
    title: '选择需要导入的Asar文件',
    buttonLabel: '导入更新文件',
    filters: [{ name: 'app.asar', extensions: ['asar'] }],
    properties: ['openFile', 'showHiddenFiles', 'noResolveAliases', 'treatPackageAsDirectory', 'dontAddToRecent']
  }, async (files: string[] | undefined) => {
    if (files && files.length > 0) {
      // 导入到app.new
      await fs.promises.cp(files[0], getAppNewPath())
      message.info('导入更新文件成功，重新打开应用...', 0)
      await Sleep(1000)
      window.WebToElectron({ cmd: 'relaunch' })
    }
  })
}
</script>

<template>
  <div class='settingcard'>
    <div class='settings-app-hero'>
      <div class='settings-app-badge'>Application</div>
      <div class='appver'>BoxPlayer {{ getAppVersion }} <span class="appver-badge" :class="{ pro: isPro }">{{ isPro ? 'PRO' : '开源版' }}</span></div>
      <div class='settings-app-subtitle'>统一配置桌面外观、启动行为、更新策略与系统集成体验</div>
    </div>
    <div class='settings-app-actions'>
      <a-button type='outline' status='success' size='small' tabindex='-1' @click='handleUpdateLog'>
        更新日志
      </a-button>
      <a-button style='margin-left: 10px' type='outline' size='small' tabindex='-1' :loading='verLoading'
                @click='handleCheckVer'>
        检查更新
      </a-button>
      <a-button style='margin-left: 10px'
                v-if='platform !== "linux"'
                status='warning' type='outline' size='small' tabindex='-1'
                @click='handleImportAsar'>
        手动导入
      </a-button>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>账号登录</div>
    <div class='settingrow' style='flex-direction:column;align-items:stretch;gap:10px'>
      <template v-if="isLoggedIn">
        <div style='display:flex;align-items:center;gap:10px'>
          <span style='font-size:14px;font-weight:500;color:var(--color-text-1)'>{{ accountEmail }}</span>
          <button class="arco-btn arco-btn-outline arco-btn-size-small" @click="handleLogout"><LogOut :size="13" /> 退出</button>
        </div>
        <button v-if="!isPro" class="arco-btn arco-btn-primary" @click="showUpgradeModal = true" style="width:100%;height:40px;font-size:14px">升级到专业版 — $10/月</button>
        <span v-else style="font-size:12px;color:rgb(var(--success-6));font-weight:600">✓ 已是专业版</span>
      </template>
      <template v-else>
        <div style='display:flex;gap:10px'>
          <button class="sa-provider sa-gh" :disabled="loading" @click="handleOAuth('github')" title="GitHub"><Github :size="20" /><span style="margin-left:6px">GitHub</span></button>
          <button class="sa-provider sa-go" :disabled="loading" @click="handleOAuth('google')" title="Google"><Chrome :size="20" /><span style="margin-left:6px">Google</span></button>
          <button class="sa-provider sa-em" :class="{ active: showEmail }" :disabled="loading" @click="showEmail = !showEmail" title="邮箱"><Mail :size="20" /><span style="margin-left:6px">邮箱</span></button>
        </div>
      </template>
      <div v-if="showEmail && !isLoggedIn" style='display:flex;gap:8px'>
        <template v-if="!codeSent">
          <input v-model="authEmail" type="email" placeholder="邮箱地址" class="sa-input" style='flex:1' />
          <button class="sa-send-btn" :disabled="loading || !authEmail.trim()" @click="handleEmailSend">
            <Loader2 v-if="loading" :size="13" class="spin" /><span v-else>发送验证码</span>
          </button>
        </template>
        <template v-else>
          <input v-model="emailCode" type="text" placeholder="验证码" maxlength="6" class="sa-input" style='flex:1' />
          <button class="sa-send-btn" :disabled="loading || emailCode.length < 4" @click="handleEmailVerify">
            <Loader2 v-if="loading" :size="13" class="spin" /><span v-else>验证并登录</span>
          </button>
        </template>
      </div>
    </div>

    <div class='settingspace'></div>
    <div class='settinghead'>界面颜色</div>
    <div class='settingrow'>
      <a-radio-group type='button' tabindex='-1' :model-value='settingStore.uiTheme'
                     @update:model-value='cb({ uiTheme: $event })'>
        <a-radio tabindex='-1' value='system'>跟随系统</a-radio>
        <a-radio tabindex='-1' value='light'>浅色模式</a-radio>
        <a-radio tabindex='-1' value='dark'>深色模式</a-radio>
      </a-radio-group>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>默认启动 Tab</div>
    <div class='settingrow'>
      <a-radio-group
        type='button'
        tabindex='-1'
        :model-value='settingStore.uiDefaultTab'
        @update:model-value='cb({ uiDefaultTab: $event })'
      >
        <a-radio tabindex='-1' value='pan'>网盘</a-radio>
        <a-radio tabindex='-1' value='media-server'>媒体服务器</a-radio>
        <a-radio tabindex='-1' value='media'>媒体库</a-radio>
      </a-radio-group>
    </div>
    <template v-if="['win32', 'darwin'].includes(platform)">
      <div class='settingspace'></div>
      <div class='settinghead'>开机自启设置</div>
      <div class='settingrow'>
        <MySwitch :value='settingStore.uiLaunchStart' @update:value='cb({ uiLaunchStart: $event })'>
          开机时自动启动
        </MySwitch>
      </div>
      <div class='settingrow' v-if="settingStore.uiLaunchStart">
        <MySwitch :value='settingStore.uiLaunchStartShow'
                  @update:value='cb({ uiLaunchStartShow: $event })'>
          自动启动后显示主窗口
        </MySwitch>
      </div>
    </template>
    <div class='settingspace'></div>
    <div class='settinghead'>检查更新设置</div>
    <div class='settingrow'>
      <MySwitch :value='settingStore.uiLaunchAutoCheckUpdate'
                @update:value='cb({ uiLaunchAutoCheckUpdate: $event })'>
        启动时检查更新
      </MySwitch>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>自动签到设置</div>
    <div class='settingrow'>
      <MySwitch :value='settingStore.uiLaunchAutoSign' @update:value='cb({ uiLaunchAutoSign: $event })'>
        启动时自动签到
      </MySwitch>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>关闭时彻底退出</div>
    <div class='settingrow'>
      <MySwitch :value='settingStore.uiExitOnClose' @update:value='cb({ uiExitOnClose: $event })'>
        关闭窗口时彻底退出小白羊
      </MySwitch>
      <a-popover position='right'>
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class='opred'>关闭</span>
            <hr />
            默认是点击窗口上的关闭按钮时<br />最小化到托盘，继续上传/下载<br /><br />开启此设置后直接彻底退出小白羊程序
          </div>
        </template>
      </a-popover>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>软件更新代理</div>
    <div class='settingrow'>
      <MySwitch :value='settingStore.uiUpdateProxyEnable' @update:value='cb({ uiUpdateProxyEnable: $event })'>
        开启软件更新代理
      </MySwitch>
      <div class='settingrow' v-if="settingStore.uiUpdateProxyEnable">
        <a-input v-model.trim='settingStore.uiUpdateProxyUrl'
                 allow-clear
                 :style="{ width: '280px' }"
                 placeholder='软件更新代理'
                 @update:model-value='cb({ uiUpdateProxyUrl: $event })' />
      </div>
    </div>
  </div>
  <LimitReachedModal :visible="showUpgradeModal" @update:visible="showUpgradeModal = $event" />
</template>

<style scoped>
.settings-app-hero {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 14px;
}

.settings-app-badge {
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

.appver {
  font-weight: 600;
  font-size: 28px;
  line-height: 1.4;
}

.settings-app-subtitle {
  max-width: 520px;
  color: var(--color-text-2);
  font-size: 14px;
  line-height: 1.7;
}

.settings-app-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.settings-app-actions :deep(.arco-btn) {
  margin-left: 0 !important;
}

:global(html.dark) .settings-app-badge {
  background: rgba(120, 160, 255, 0.2);
  color: #dbe6ff;
}

@media (max-width: 900px) {
  .appver {
    font-size: 24px;
  }
}

.appver-badge{display:inline-block;margin-left:10px;padding:3px 12px;font-size:12px;font-weight:700;letter-spacing:.05em;color:var(--color-text-3);background:var(--color-fill-2);border:1px solid var(--color-border);border-radius:6px;vertical-align:middle}
.appver-badge.pro{color:#b45309;background:rgba(245,158,11,.15);border-color:rgba(245,158,11,.4);box-shadow:0 1px 3px rgba(245,158,11,.15)}
.appver-actions{display:flex;align-items:center;gap:8px;margin-top:8px}
.appver-email{font-size:12px;color:var(--color-text-3)}
.appver-login{padding:3px 10px;font-size:11px;color:rgb(var(--primary-6));background:transparent;border:1px solid rgb(var(--primary-6));border-radius:5px;cursor:pointer;font-family:inherit}
.appver-login:hover{background:rgba(var(--primary-6),.08)}
.appver-logout{padding:3px 10px;font-size:11px;color:var(--color-text-4);background:transparent;border:1px solid var(--color-border);border-radius:5px;cursor:pointer;font-family:inherit}
.appver-logout:hover{color:rgb(var(--danger-6));border-color:rgb(var(--danger-6))}
.appver-upgrade{padding:3px 12px;font-size:11px;font-weight:600;color:#fff;background:linear-gradient(135deg,#f59e0b,#eab308);border:0;border-radius:6px;cursor:pointer;font-family:inherit}
.appver-upgrade:hover{opacity:.9}

.sa-provider{display:flex;align-items:center;justify-content:center;gap:6px;padding:9px 16px;font-size:13px;font-weight:500;border:1px solid var(--color-border);border-radius:8px;cursor:pointer;font-family:inherit;transition:all .15s;flex:1}
.sa-provider:hover:not(:disabled){transform:translateY(-1px)}
.sa-provider:disabled{opacity:.4;cursor:default}
.sa-gh{background:#24292e;color:#fff;border-color:#24292e}
.sa-gh:hover:not(:disabled){box-shadow:0 2px 8px rgba(36,41,46,.3)}
.sa-go{background:var(--color-bg-1);color:var(--color-text-3)}
.sa-go:hover:not(:disabled){color:var(--color-text-1);border-color:var(--color-border-2)}
.sa-em{background:var(--color-bg-1);color:var(--color-text-3)}
.sa-em:hover:not(:disabled),.sa-em.active{color:rgb(var(--primary-6));border-color:rgb(var(--primary-6))}
.sa-input{padding:6px 10px;font-size:12px;color:var(--color-text-1);background:var(--color-fill-1);border:1px solid var(--color-border);border-radius:6px;outline:none;font-family:inherit}
.sa-input:focus{border-color:rgb(var(--primary-6))}
.sa-send-btn{display:flex;align-items:center;gap:4px;padding:6px 12px;font-size:12px;font-weight:500;color:#fff;background:rgb(var(--primary-6));border:0;border-radius:6px;cursor:pointer;font-family:inherit;white-space:nowrap;flex-shrink:0}
.sa-send-btn:hover:not(:disabled){opacity:.9}
.sa-send-btn:disabled{opacity:.4;cursor:default}

.spin{animation:spin 1s linear infinite}
@keyframes spin{to{transform:rotate(360deg)}}
</style>
