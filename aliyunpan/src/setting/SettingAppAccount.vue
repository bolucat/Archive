<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { Github, Chrome, Mail, Loader2, LogOut } from 'lucide-vue-next'
import { createClient } from '@supabase/supabase-js'
import Config from '../config'
import message from '../utils/message'
import { openExternal } from '../utils/electronhelper'

const loading = ref(false)
const emailInput = ref('')
const showEmail = ref(false)
const codeSent = ref(false)
const emailCode = ref('')
const userEmail = ref('')
const isLoggedIn = ref(false)
const isPro = ref(false)
try {
  userEmail.value = localStorage.getItem('app_user_email') || ''
  isLoggedIn.value = localStorage.getItem('app_user_authed') === '1'
  isPro.value = localStorage.getItem('app_user_pro') === '1'
} catch {}
const upgrading = ref(false)
const showUpgradeModal = ref(false)

const CALLBACK_URL = 'boxplayer-auth://callback'
const PAYMENT_CALLBACK = 'boxplayer-auth://payment-success'

const supabase = Config.SUPABASE_URL && Config.SUPABASE_ANON_KEY
  ? createClient(Config.SUPABASE_URL, Config.SUPABASE_ANON_KEY)
  : null

function saveLogin(email: string) {
  localStorage.setItem('app_user_email', email)
  localStorage.setItem('app_user_authed', '1')
  userEmail.value = email
  isLoggedIn.value = true
}

function handleLogout() {
  localStorage.removeItem('app_user_email')
  localStorage.removeItem('app_user_authed')
  supabase?.auth.signOut().catch(() => {})
  userEmail.value = ''
  isLoggedIn.value = false
  message.success('已退出登录')
}

async function handleOAuth(provider: 'github' | 'google') {
  if (!supabase) { message.error('未配置 Supabase，请在 config.ts 填入 SUPABASE_URL 和 SUPABASE_ANON_KEY'); return }
  loading.value = true
  try {
    const { data, error } = await supabase.auth.signInWithOAuth({
      provider,
      options: { redirectTo: CALLBACK_URL, skipBrowserRedirect: true },
    })
    if (error) message.error(error.message)
    else if (data.url) {
      openExternal(data.url)
    }
  } finally { loading.value = false }
}

async function handleEmailSend() {
  const email = emailInput.value.trim()
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
  const email = emailInput.value.trim()
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
  if (!Config.CREEM_API_KEY || !Config.CREEM_PRODUCT_ID) {
    message.error('Creem 未配置，请在 config.ts 填入 CREEM_API_KEY 和 CREEM_PRODUCT_ID'); return
  }
  upgrading.value = true
  try {
    const email = userEmail.value || emailInput.value.trim()
    const apiBase = Config.CREEM_API_KEY.startsWith('creem_test_') ? 'https://test-api.creem.io' : 'https://api.creem.io'
    const resp = await fetch(`${apiBase}/v1/checkouts`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'x-api-key': Config.CREEM_API_KEY },
      body: JSON.stringify({
        product_id: Config.CREEM_PRODUCT_ID,
        success_url: PAYMENT_CALLBACK,
        customer: { email: email || undefined },
        metadata: { app_user: 'boxplayer' },
      }),
    })
    const data = await resp.json()
    if (data.checkout_url) {
      openExternal(data.checkout_url)
    } else {
      message.error(data.message || '创建支付链接失败')
    }
  } catch (e: any) { message.error(e?.message || '网络请求失败') }
  finally { upgrading.value = false }
}

function setupPaymentCallback() {
  if (!window.Electron?.ipcRenderer) return
  const handler = async (_e: any, params: { checkout_id?: string }) => {
    if (params.checkout_id && Config.CREEM_API_KEY) {
      try {
        const apiBase = Config.CREEM_API_KEY.startsWith('creem_test_') ? 'https://test-api.creem.io' : 'https://api.creem.io'
        const resp = await fetch(`${apiBase}/v1/checkouts/${params.checkout_id}`, {
          headers: { 'x-api-key': Config.CREEM_API_KEY },
        })
        const data = await resp.json()
        if (data?.status === 'completed') {
          localStorage.setItem('app_user_pro', '1')
          isPro.value = true
          message.success('Pro 已激活！')
        }
      } catch {}
    }
  }
  window.Electron.ipcRenderer.on('payment-callback', handler)
}

function setupCallbackListener() {
  if (!window.Electron?.ipcRenderer) return
  const handler = async (_e: any, params: { access_token?: string; refresh_token?: string }) => {
    if (!params.access_token || !supabase) return
    const { data, error } = await supabase.auth.setSession({
      access_token: params.access_token,
      refresh_token: params.refresh_token || '',
    })
    if (!error && data.user) { saveLogin(data.user.email || ''); message.success('登录成功') }
  }
  window.Electron.ipcRenderer.on('auth-callback', handler)
  onUnmounted(() => window.Electron.ipcRenderer?.removeListener('auth-callback', handler))
}

onMounted(() => {
  setupCallbackListener()
  setupPaymentCallback()
  if (localStorage.getItem('boxplayer_show_pricing') === '1') {
    localStorage.removeItem('boxplayer_show_pricing')
    if (!isPro.value) showUpgradeModal.value = true
  }
})
</script>

<template>
  <div id="SettingAppAccount" class="setting-section">
    <div class="sa-header">
      <span class="sa-title">应用账户</span>
      <span v-if="!isLoggedIn" class="sa-hint">GitHub / Google / 邮箱登录</span>
    </div>

    <div class="sa-section-title">账号登录</div>

    <template v-if="isLoggedIn">
      <div class="sa-logged">
        <span class="sa-email">{{ userEmail }}</span>
        <span v-if="isPro" class="sa-pro-badge">PRO</span>
        <button class="sa-logout" @click="handleLogout"><LogOut :size="13" /> 退出</button>
      </div>
    </template>

    <template v-else>
      <div class="sa-oauth">
        <button class="sa-provider sa-gh" :disabled="loading" @click="handleOAuth('github')" title="GitHub"><Github :size="20" /></button>
        <button class="sa-provider sa-go" :disabled="loading" @click="handleOAuth('google')" title="Google"><Chrome :size="20" /></button>
        <button class="sa-provider sa-em" :class="{ active: showEmail }" :disabled="loading" @click="showEmail = !showEmail" title="邮箱"><Mail :size="20" /></button>
      </div>

      <div v-if="showEmail" class="sa-email-box">
        <template v-if="!codeSent">
          <input v-model="emailInput" type="email" placeholder="邮箱地址" />
          <button :disabled="loading || !emailInput.trim()" @click="handleEmailSend"><Loader2 v-if="loading" :size="13" class="spin" /><span v-else>发送验证码</span></button>
        </template>
        <template v-else>
          <input v-model="emailCode" type="text" placeholder="输入验证码" maxlength="6" @keydown.enter="handleEmailVerify" />
          <button :disabled="loading || emailCode.length < 4" @click="handleEmailVerify"><Loader2 v-if="loading" :size="13" class="spin" /><span v-else>验证并登录</span></button>
        </template>
      </div>
    </template>

    <!-- Version badge + upgrade -->
    <div class="sa-version">
      <div class="sa-version-badge" :class="{ pro: isPro }">
        <span v-if="isPro">专业版 PRO</span>
        <span v-else>开源版</span>
      </div>
      <button v-if="!isPro" class="sa-version-upgrade" @click="showUpgradeModal = true">升级到专业版</button>
    </div>

    <!-- Upgrade modal -->
    <div v-if="showUpgradeModal" class="upg-mask" @click.self="showUpgradeModal = false">
      <div class="upg-modal">
        <button class="upg-close" @click="showUpgradeModal = false"><X :size="18" /></button>
        <h2 class="upg-title">版本对比</h2>

        <table class="upg-table">
          <thead>
            <tr><th>功能</th><th>开源版</th><th class="upg-th-pro">专业版</th></tr>
          </thead>
          <tbody>
            <tr><td>网盘文件管理</td><td class="upg-yes">✓</td><td class="upg-yes">✓</td></tr>
            <tr><td>视频 / 音乐播放</td><td class="upg-yes">✓</td><td class="upg-yes">✓</td></tr>
            <tr><td>本地书籍阅读</td><td class="upg-yes">✓</td><td class="upg-yes">✓</td></tr>
            <tr><td>多网盘同时连接</td><td class="upg-yes">✓</td><td class="upg-yes">✓</td></tr>
            <tr><td>AI 智能搜索</td><td class="upg-limit">5次/天</td><td class="upg-yes">无限</td></tr>
            <tr><td>全网资源搜索</td><td class="upg-limit">5次/天</td><td class="upg-yes">无限</td></tr>
            <tr><td>AI 文件整理 & 查重</td><td class="upg-no">✗</td><td class="upg-yes">✓</td></tr>
            <tr><td>AI 阅读助手</td><td class="upg-no">✗</td><td class="upg-yes">✓</td></tr>
            <tr><td>语音朗读</td><td class="upg-no">✗</td><td class="upg-yes">✓</td></tr>
            <tr><td>即时翻译</td><td class="upg-no">✗</td><td class="upg-yes">✓</td></tr>
            <tr><td>全网资源一键保存</td><td class="upg-no">✗</td><td class="upg-yes">✓</td></tr>
            <tr><td>TMDB + 豆瓣电影发现</td><td class="upg-no">✗</td><td class="upg-yes">✓</td></tr>
            <tr><td>优先技术支持</td><td class="upg-no">✗</td><td class="upg-yes">✓</td></tr>
          </tbody>
        </table>

        <div class="upg-price">$10 / 月</div>

        <button v-if="isLoggedIn" class="upg-btn" :disabled="upgrading" @click="handleUpgrade">
          <Loader2 v-if="upgrading" :size="14" class="spin" /> <span v-else>升级到专业版</span>
        </button>
        <button v-else class="upg-btn upg-btn-login" @click="message.info('请先使用上方 GitHub / Google / 邮箱登录后再购买'); showUpgradeModal = false">
          登录后购买
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.setting-section { padding: 8px 0; }
.sa-header { display: flex; align-items: baseline; gap: 10px; margin-bottom: 12px; }
.sa-title { font-size: 15px; font-weight: 700; color: var(--color-text-1); }
.sa-hint { font-size: 12px; color: var(--color-text-4); }

.sa-logged { display: flex; align-items: center; gap: 12px; padding: 8px 12px; background: var(--color-fill-1); border: 1px solid var(--color-border); border-radius: 8px; margin-bottom: 14px; }
.sa-email { font-size: 14px; font-weight: 500; color: var(--color-text-1); }
.sa-pro-badge { padding: 1px 8px; font-size: 10px; font-weight: 800; letter-spacing: .1em; color: #fff; background: linear-gradient(135deg, #f59e0b, #eab308); border-radius: 4px; }
.sa-logout { display: flex; align-items: center; gap: 4px; margin-left: auto; padding: 4px 10px; font-size: 11px; color: var(--color-text-4); background: transparent; border: 1px solid var(--color-border); border-radius: 5px; cursor: pointer; font-family: inherit; }
.sa-logout:hover { color: rgb(var(--danger-6)); border-color: rgb(var(--danger-6)); }

.sa-oauth { display: flex; gap: 12px; margin-bottom: 14px; }
.sa-provider { display: flex; align-items: center; justify-content: center; width: 40px; height: 40px; padding: 0; border: 1px solid var(--color-border); border-radius: 50%; cursor: pointer; font-family: inherit; transition: all .15s; }
.sa-provider:hover:not(:disabled) { transform: translateY(-1px); }
.sa-provider:disabled { opacity: .4; cursor: default; }
.sa-gh { background: #24292e; color: #fff; border-color: #24292e; }
.sa-gh:hover:not(:disabled) { box-shadow: 0 2px 8px rgba(36,41,46,.3); }
.sa-go { background: var(--color-bg-1); color: var(--color-text-3); }
.sa-go:hover:not(:disabled) { color: var(--color-text-1); border-color: var(--color-border-2); }
.sa-em { background: var(--color-bg-1); color: var(--color-text-3); }
.sa-em:hover:not(:disabled), .sa-em.active { color: rgb(var(--primary-6)); border-color: rgb(var(--primary-6)); }

.sa-email-box { display: flex; gap: 8px; margin-bottom: 14px; padding-bottom: 8px; border-bottom: 1px solid var(--color-border); }
.sa-email-box input { flex: 1; padding: 7px 10px; font-size: 12px; color: var(--color-text-1); background: var(--color-fill-1); border: 1px solid var(--color-border); border-radius: 7px; outline: none; font-family: inherit; min-width: 0; }
.sa-email-box input:focus { border-color: rgb(var(--primary-6)); }
.sa-email-box button { display: flex; align-items: center; gap: 3px; padding: 7px 12px; font-size: 12px; font-weight: 500; color: #fff; background: rgb(var(--primary-6)); border: 0; border-radius: 7px; cursor: pointer; font-family: inherit; white-space: nowrap; }
.sa-email-box button:hover:not(:disabled) { opacity: .9; }
.sa-email-box button:disabled { opacity: .4; cursor: default; }

/* Section title */
.sa-section-title { font-size: 13px; font-weight: 600; color: var(--color-text-3); margin-bottom: 8px; padding-bottom: 6px; border-bottom: 1px solid var(--color-border); }

/* Version badge */
.sa-version { display: flex; align-items: center; gap: 10px; margin-top: 10px; }
.sa-version-badge { padding: 3px 10px; font-size: 11px; font-weight: 700; color: var(--color-text-2); background: var(--color-fill-2); border: 1px solid var(--color-border); border-radius: 6px; }
.sa-version-badge.pro { color: #b45309; background: rgba(245,158,11,.15); border-color: rgba(245,158,11,.3); }
.sa-version-upgrade { margin-left: auto; padding: 6px 14px; font-size: 12px; font-weight: 600; color: #fff; background: linear-gradient(135deg,#f59e0b,#eab308); border:0; border-radius:7px; cursor:pointer; font-family:inherit; }
.sa-version-upgrade:hover { opacity:.9; }

/* Upgrade modal */
.upg-mask{position:fixed;inset:0;background:rgba(0,0,0,.5);backdrop-filter:blur(3px);display:flex;align-items:center;justify-content:center;z-index:2000}
.upg-modal{position:relative;width:620px;max-width:94vw;max-height:90vh;overflow-y:auto;background:var(--color-bg-2);border:1px solid var(--color-border);border-radius:16px;padding:28px 24px 20px;box-shadow:0 16px 48px rgba(0,0,0,.2)}
.upg-close{position:absolute;top:12px;right:12px;display:flex;align-items:center;justify-content:center;width:28px;height:28px;padding:0;color:var(--color-text-4);background:transparent;border:0;border-radius:6px;cursor:pointer}
.upg-close:hover{background:var(--color-fill-2);color:var(--color-text-1)}
.upg-title{font-size:18px;font-weight:700;color:var(--color-text-1);text-align:center;margin:0 0 16px}
.upg-table{width:100%;border-collapse:collapse;margin-bottom:16px}
.upg-table th,.upg-table td{padding:8px 12px;font-size:12px;text-align:center;border-bottom:1px solid var(--color-border)}
.upg-table th{color:var(--color-text-3);font-weight:600}
.upg-table th:first-child,.upg-table td:first-child{text-align:left;color:var(--color-text-2)}
.upg-th-pro{color:#b45309 !important}
.upg-yes{color:rgb(var(--success-6));font-weight:700}
.upg-no{color:var(--color-text-4)}
.upg-limit{color:var(--color-text-3);font-size:11px}
.upg-price{text-align:center;font-size:22px;font-weight:700;color:rgb(var(--primary-6));margin-bottom:12px}
.upg-btn{display:block;width:100%;padding:10px 0;font-size:14px;font-weight:600;color:#fff;background:linear-gradient(135deg,#f59e0b,#eab308);border:0;border-radius:8px;cursor:pointer;font-family:inherit;display:flex;align-items:center;justify-content:center;gap:6px}
.upg-btn:hover:not(:disabled){opacity:.9}
.upg-btn:disabled{opacity:.5;cursor:default}
.upg-btn-login{background:var(--color-text-4)}

.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>
