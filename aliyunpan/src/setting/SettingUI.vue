<script setup lang='ts'>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import useSettingStore from './settingstore'
import MySwitch from '../layout/MySwitch.vue'
import LimitReachedModal from './LimitReachedModal.vue'
import { createClient } from '@supabase/supabase-js'
import { openExternal } from '../utils/electronhelper'
import { CheckCircle2, Chrome, Crown, Github, Loader2, LogOut, Mail, RefreshCw } from 'lucide-vue-next'
import ServerHttp from '../aliapi/server'
import os from 'os'
import { getAppNewPath, getResourcesPath } from '../utils/electronhelper'
import { existsSync, readFileSync } from 'fs'
import { getPkgVersion } from '../utils/utils'
import { modalUpdateLog } from '../utils/modal'
import fs from 'node:fs'
import message from '../utils/message'
import { Sleep } from '../utils/format'

const SUPABASE_URL = 'https://ltqipofjjqjlbbfsgihi.supabase.co'
const SUPABASE_ANON_KEY = 'sb_publishable_VzoE4CzxiTaNpFVkFUc8cA_XARw0T3r'
const BOXPLAYER_SITE_URL = 'https://xbysite.pages.dev'

const platform = window.platform
const settingStore = useSettingStore()

const isPro = ref(false)
const isLoggedIn = ref(false)
const accountEmail = ref('')
const accountInitial = computed(() => (accountEmail.value.trim().charAt(0) || 'B').toUpperCase())
try {
  isPro.value = localStorage.getItem('app_user_pro') === '1'
  isLoggedIn.value = localStorage.getItem('app_user_authed') === '1'
  accountEmail.value = localStorage.getItem('app_user_email') || ''
} catch {}
const showUpgradeModal = ref(false)

onMounted(() => {
  setupAuthCallback()
  setupPaymentCallback()
  if (isLoggedIn.value) syncProStatus()
  if (localStorage.getItem('boxplayer_show_pricing') === '1') {
    localStorage.removeItem('boxplayer_show_pricing')
    if (!isPro.value) showUpgradeModal.value = true
  }
})

onUnmounted(() => {
  if (!window.Electron?.ipcRenderer) return
  if (authCallbackHandler) window.Electron.ipcRenderer.removeListener('auth-callback', authCallbackHandler)
  if (paymentCallbackHandler) window.Electron.ipcRenderer.removeListener('payment-callback', paymentCallbackHandler)
})

function handleLogout() {
  localStorage.removeItem('app_user_email')
  localStorage.removeItem('app_user_authed')
  localStorage.removeItem('app_user_pro')
  isLoggedIn.value = false
  isPro.value = false
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
const PAYMENT_POLL_DELAYS = [0, 2000, 5000, 10000, 20000]

const supabase = SUPABASE_URL && SUPABASE_ANON_KEY
  ? createClient(SUPABASE_URL, SUPABASE_ANON_KEY) : null

function saveLogin(email: string) {
  localStorage.setItem('app_user_email', email)
  localStorage.setItem('app_user_authed', '1')
  accountEmail.value = email
  isLoggedIn.value = true
  syncProStatus()
}

async function syncProStatus() {
  if (!isLoggedIn.value || !supabase) return false
  try {
    const { data } = await supabase.auth.getSession()
    const token = data.session?.access_token
    if (!token) return false
    const response = await fetch(`${BOXPLAYER_SITE_URL}/api/me/subscription`, { headers: { Authorization: `Bearer ${token}` } })
    if (!response.ok) return false
    const subscription = await response.json()
    isPro.value = subscription.isPro === true
    if (isPro.value) {
      localStorage.setItem('app_user_pro', '1')
    } else {
      localStorage.removeItem('app_user_pro')
    }
    return isPro.value
  } catch {}
  return false
}

async function pollProStatusAfterPayment() {
  if (!isLoggedIn.value) {
    message.warning('支付完成，请先登录后同步专业版状态')
    return
  }

  for (let i = 0; i < PAYMENT_POLL_DELAYS.length; i += 1) {
    const delay = PAYMENT_POLL_DELAYS[i]
    if (delay > 0) await Sleep(delay)
    const active = await syncProStatus()
    if (active) {
      message.success('Pro 已激活！')
      return
    }
  }

  message.warning('支付已完成，专业版授权仍在同步中，请稍后点击同步状态')
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
  if (!supabase) { message.error('未配置 Supabase'); return }
  upgrading.value = true
  try {
    const { data } = await supabase.auth.getSession()
    const token = data.session?.access_token
    if (!token) { message.warning('请先登录后升级'); return }
    const response = await fetch(`${BOXPLAYER_SITE_URL}/api/creem/checkout`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ cycle: 'lifetime', source: 'app' })
    })
    const checkout = await response.json()
    if (!response.ok || !checkout.checkoutUrl) throw new Error(checkout.error || '创建支付订单失败')
    openExternal(checkout.checkoutUrl)
  } catch (e: any) { message.error(e?.message || '网络请求失败') }
  finally { upgrading.value = false }
}

let authCallbackHandler: ((_e: any, params: { access_token?: string; refresh_token?: string }) => void) | null = null
let paymentCallbackHandler: ((_e: any, params?: { status?: string; checkout_id?: string; reason?: string }) => void) | null = null

function setupAuthCallback() {
  if (!window.Electron?.ipcRenderer) return
  authCallbackHandler = async (_e: any, params: { access_token?: string; refresh_token?: string }) => {
    if (!params.access_token || !supabase) return
    const { data, error } = await supabase.auth.setSession({ access_token: params.access_token, refresh_token: params.refresh_token || '' })
    if (!error && data.user) { saveLogin(data.user.email || ''); message.success('登录成功') }
  }
  window.Electron.ipcRenderer.on('auth-callback', authCallbackHandler)
}

function setupPaymentCallback() {
  if (!window.Electron?.ipcRenderer) return
  paymentCallbackHandler = async (_e: any, params?: { status?: string; checkout_id?: string; reason?: string }) => {
    const status = params?.status || 'success'
    if (status === 'cancelled' || status === 'canceled') {
      message.info('已取消购买，未产生专业版授权')
      return
    }
    if (status === 'failed' || status === 'failure') {
      message.error('支付未完成，请重试或稍后再试')
      return
    }

    message.info('支付完成，正在同步专业版状态…')
    await pollProStatusAfterPayment()
  }
  window.Electron.ipcRenderer.on('payment-callback', paymentCallbackHandler)
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
      <a-button type='outline' status='success' size='small' @click='handleUpdateLog'>
        更新日志
      </a-button>
      <a-button type='outline' size='small' :loading='verLoading' @click='handleCheckVer'>
        检查更新
      </a-button>
      <a-button v-if='platform !== "linux"' status='warning' type='outline' size='small' @click='handleImportAsar'>
        手动导入
      </a-button>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>账号与专业版</div>
    <div class='settingrow setting-account-row'>
      <div class='setting-account-copy'>
        <div class='setting-account-title'>BoxPlayer 账号</div>
        <div class='setting-account-desc'>登录后同步专业版状态，终身授权暂时覆盖 Windows、Linux、MacOS。</div>
      </div>

      <div class='setting-account-panel'>
        <template v-if="isLoggedIn">
          <div class='setting-account-main'>
            <div class='setting-account-avatar' aria-hidden='true'>{{ accountInitial }}</div>
            <div class='setting-account-identity'>
              <div class='setting-account-email'>{{ accountEmail }}</div>
              <div class='setting-account-meta'>
                <span class='setting-pro-badge' :class="{ active: isPro }">
                  <CheckCircle2 v-if="isPro" :size='13' />
                  <Crown v-else :size='13' />
                  {{ isPro ? '专业版已激活' : '开源版' }}
                </span>
              </div>
            </div>
            <button class='setting-icon-btn' title='同步状态' :disabled='loading' @click='syncProStatus'>
              <RefreshCw :size='15' />
            </button>
            <button class='setting-icon-btn danger' title='退出登录' @click='handleLogout'>
              <LogOut :size='15' />
            </button>
          </div>

          <button v-if="!isPro" class='setting-upgrade-btn' :disabled='upgrading' @click='handleUpgrade'>
            <Loader2 v-if="upgrading" :size='15' class='spin' />
            <Crown v-else :size='15' />
            <span>{{ upgrading ? '正在创建订单' : '购买终身专业版 — $139' }}</span>
          </button>
        </template>

        <template v-else>
          <div class='setting-account-main'>
            <div class='setting-account-avatar muted' aria-hidden='true'>B</div>
            <div class='setting-account-identity'>
              <div class='setting-account-email'>未登录</div>
              <div class='setting-account-meta'>选择一种方式登录后再购买专业版</div>
            </div>
          </div>

          <div class='setting-provider-grid'>
            <button class="sa-provider sa-gh" :disabled="loading" title="GitHub" @click="handleOAuth('github')">
              <Github :size="18" />
              <span>GitHub</span>
            </button>
            <button class="sa-provider sa-go" :disabled="loading" title="Google" @click="handleOAuth('google')">
              <Chrome :size="18" />
              <span>Google</span>
            </button>
            <button class="sa-provider sa-em" :class="{ active: showEmail }" :disabled="loading" title="邮箱" @click="showEmail = !showEmail">
              <Mail :size="18" />
              <span>邮箱</span>
            </button>
          </div>

          <div v-if="showEmail" class='setting-email-form'>
            <template v-if="!codeSent">
              <input v-model="authEmail" type="email" placeholder="邮箱地址" class="sa-input" @keydown.enter="handleEmailSend" />
              <button class="sa-send-btn" :disabled="loading || !authEmail.trim()" @click="handleEmailSend">
                <Loader2 v-if="loading" :size="13" class="spin" />
                <span v-else>发送验证码</span>
              </button>
            </template>
            <template v-else>
              <input v-model="emailCode" type="text" placeholder="验证码" maxlength="6" class="sa-input" @keydown.enter="handleEmailVerify" />
              <button class="sa-send-btn" :disabled="loading || emailCode.length < 4" @click="handleEmailVerify">
                <Loader2 v-if="loading" :size="13" class="spin" />
                <span v-else>验证并登录</span>
              </button>
            </template>
          </div>
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
:global(html.dark) .appver-badge.pro{color:#fbbf24;background:rgba(251,191,36,.12);border-color:rgba(251,191,36,.35)}

.setting-account-row {
  align-items: flex-start;
  justify-content: space-between;
  gap: 24px;
}

.setting-account-copy {
  min-width: 180px;
  padding-top: 5px;
}

.setting-account-title {
  color: var(--color-text-1);
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
}

.setting-account-desc {
  max-width: 360px;
  margin-top: 4px;
  color: var(--color-text-3);
  font-size: 12px;
  line-height: 18px;
}

.setting-account-panel {
  width: min(100%, 460px);
  padding: 12px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-bg-1);
}

.setting-account-main {
  display: flex;
  align-items: center;
  min-height: 40px;
  gap: 10px;
}

.setting-account-avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  flex: 0 0 36px;
  border-radius: 8px;
  background: rgba(var(--primary-6), 0.12);
  color: rgb(var(--primary-6));
  font-size: 15px;
  font-weight: 700;
}

.setting-account-avatar.muted {
  background: var(--color-fill-2);
  color: var(--color-text-3);
}

.setting-account-identity {
  min-width: 0;
  flex: 1;
}

.setting-account-email {
  overflow: hidden;
  color: var(--color-text-1);
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.setting-account-meta {
  display: flex;
  align-items: center;
  min-height: 18px;
  margin-top: 2px;
  color: var(--color-text-3);
  font-size: 12px;
  line-height: 18px;
}

.setting-pro-badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  max-width: 100%;
  padding: 2px 8px;
  border: 1px solid var(--color-border);
  border-radius: 999px;
  background: var(--color-fill-1);
  color: var(--color-text-3);
  font-size: 12px;
  font-weight: 600;
  line-height: 18px;
}

.setting-pro-badge.active {
  border-color: rgba(var(--success-6), 0.35);
  background: rgba(var(--success-6), 0.1);
  color: rgb(var(--success-6));
}

.setting-icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  flex: 0 0 32px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-1);
  color: var(--color-text-3);
  font-family: inherit;
}

.setting-icon-btn:hover:not(:disabled) {
  border-color: var(--color-border-2);
  color: var(--color-text-1);
}

.setting-icon-btn.danger:hover:not(:disabled) {
  border-color: rgba(var(--danger-6), 0.4);
  color: rgb(var(--danger-6));
}

.setting-icon-btn:disabled {
  opacity: 0.45;
}

.setting-upgrade-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  min-height: 38px;
  margin-top: 12px;
  gap: 7px;
  border: 1px solid rgba(217, 119, 6, 0.35);
  border-radius: 6px;
  background: #d97706;
  color: #fff;
  font-family: inherit;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
}

.setting-upgrade-btn:hover:not(:disabled) {
  background: #b45309;
}

.setting-upgrade-btn:disabled {
  opacity: 0.65;
}

.setting-provider-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
  margin-top: 12px;
}

.sa-provider {
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 0;
  min-height: 36px;
  gap: 6px;
  padding: 0 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  font-family: inherit;
  font-size: 13px;
  font-weight: 600;
}

.sa-provider:disabled {
  opacity: 0.45;
}

.sa-gh {
  background: #24292e;
  color: #fff;
  border-color: #24292e;
}

.sa-go,
.sa-em {
  background: var(--color-bg-1);
  color: var(--color-text-2);
}

.sa-go:hover:not(:disabled),
.sa-em:hover:not(:disabled),
.sa-em.active {
  border-color: rgb(var(--primary-6));
  color: rgb(var(--primary-6));
}

.setting-email-form {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}

.sa-input {
  min-width: 0;
  flex: 1;
  height: 34px;
  padding: 0 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  outline: none;
  background: var(--color-fill-1);
  color: var(--color-text-1);
  font-family: inherit;
  font-size: 13px;
}

.sa-input:focus {
  border-color: rgb(var(--primary-6));
  background: var(--color-bg-1);
}

.sa-send-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 94px;
  height: 34px;
  flex: 0 0 auto;
  gap: 5px;
  padding: 0 12px;
  border: 0;
  border-radius: 6px;
  background: rgb(var(--primary-6));
  color: #fff;
  font-family: inherit;
  font-size: 12px;
  font-weight: 700;
  white-space: nowrap;
}

.sa-send-btn:hover:not(:disabled) {
  opacity: 0.9;
}

.sa-send-btn:disabled {
  opacity: 0.45;
}

@media (max-width: 720px) {
  .setting-account-row {
    flex-direction: column;
    gap: 10px;
  }

  .setting-account-copy,
  .setting-account-panel {
    width: 100%;
  }

  .setting-provider-grid {
    grid-template-columns: 1fr;
  }

  .setting-email-form {
    flex-direction: column;
  }

  .sa-send-btn {
    width: 100%;
  }
}

.spin{animation:spin 1s linear infinite}
@keyframes spin{to{transform:rotate(360deg)}}
</style>
