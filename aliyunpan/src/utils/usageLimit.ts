export type FeatureName = 'aiAgentChat' | 'panHubSearch' | 'panHubSave' | 'readerAIChat' | 'readerTTS' | 'readerCloudTTS' | 'readerTranslation' | 'mediaAIScrape'

export interface FeatureConfig { freeLimit: number; vipLimit: number | null; unit: string; freeMessage: string; vipMessage: string; vipOnlyMessage: string; requiresPro?: boolean }
export interface UsageCheckResult { allowed: boolean; message?: string }
export interface UsageCheckOptions { metered?: boolean; isBYOK?: boolean }

const CONFIGS: Record<FeatureName, FeatureConfig> = {
  aiAgentChat:    { freeLimit: 0, vipLimit: null, unit: 'sessions',  freeMessage: '', vipMessage: '', vipOnlyMessage: 'AI 搜索需购买 Pro 后使用', requiresPro: true },
  panHubSearch:   { freeLimit: 5, vipLimit: 5, unit: 'searches', freeMessage: '今日全网资源搜索次数已用完（5次），升级 Pro 后可无限使用', vipMessage: '今日全网资源搜索次数已用完（5次），升级 Pro 后可无限使用', vipOnlyMessage: '' },
  panHubSave:     { freeLimit: 5, vipLimit: null, unit: 'saves',    freeMessage: '今日保存次数已用完（5次），登录后可无限使用', vipMessage: '', vipOnlyMessage: '' },
  readerAIChat:   { freeLimit: 0, vipLimit: null, unit: 'sessions',   freeMessage: '', vipMessage: '', vipOnlyMessage: '阅读器 AI 对话需购买 Pro 后使用', requiresPro: true },
  readerTTS:         { freeLimit: 0, vipLimit: null, unit: 'chars',   freeMessage: '', vipMessage: '', vipOnlyMessage: '朗读功能需登录后使用' },
  readerCloudTTS:    { freeLimit: 0, vipLimit: null, unit: 'chars',   freeMessage: '', vipMessage: '', vipOnlyMessage: '云端高品质朗读需购买 Pro 后使用', requiresPro: true },
  readerTranslation: { freeLimit: 0, vipLimit: null, unit: 'chars', freeMessage: '', vipMessage: '', vipOnlyMessage: '翻译功能需购买 Pro 后使用', requiresPro: true },
  mediaAIScrape: { freeLimit: 0, vipLimit: null, unit: 'sessions', freeMessage: '', vipMessage: '', vipOnlyMessage: 'AI 影视刮削需购买 Pro 后使用', requiresPro: true },
}

let cleanedToday = false

function today(): string { return new Date().toISOString().slice(0, 10) }

function getKey(feature: string): string { return `usage_${today()}_${feature}` }

function cleanOldKeys(): void {
  if (cleanedToday) return
  cleanedToday = true
  const d = today()
  try {
    const keys: string[] = []
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i)
      if (k?.startsWith('usage_') && !k.startsWith(`usage_${d}_`)) keys.push(k)
    }
    keys.forEach(k => localStorage.removeItem(k))
  } catch {}
}

export function isVIP(): boolean {
  try {
    return localStorage.getItem('app_user_authed') === '1' || localStorage.getItem('app_user_pro') === '1'
  } catch { return false }
}

export function isLoggedIn(): boolean {
  try {
    return localStorage.getItem('app_user_authed') === '1'
  } catch { return false }
}

export function isPro(): boolean {
  try {
    return localStorage.getItem('app_user_pro') === '1'
  } catch { return false }
}

export function getUsageCount(feature: FeatureName): number {
  try { const v = parseInt(localStorage.getItem(getKey(feature)) || '0', 10); return Number.isFinite(v) ? v : 0 } catch { return 0 }
}

export function incrementUsage(feature: FeatureName, amount = 1): void {
  try { localStorage.setItem(getKey(feature), String(getUsageCount(feature) + amount)) } catch {}
}

export function getRemainingCredits(feature: FeatureName): number | 'unlimited' {
  const cfg = CONFIGS[feature]
  if (cfg.requiresPro && !isPro()) return 0
  if (!isVIP() && cfg.freeLimit === 0) return 0
  const effectiveLimit = feature === 'panHubSearch' && isPro() ? null : cfg.requiresPro ? cfg.vipLimit : isVIP() ? cfg.vipLimit : cfg.freeLimit
  if (effectiveLimit === null) return 'unlimited'
  return Math.max(0, effectiveLimit - getUsageCount(feature))
}

export function checkAndIncrement(feature: FeatureName, amount = 1, options: UsageCheckOptions = {}): UsageCheckResult {
  cleanOldKeys()
  const cfg = CONFIGS[feature]
  const vip = isVIP()
  const pro = isPro()
  const metered = options.metered !== false

  if (cfg.requiresPro && !pro) {
    if (options.isBYOK) {
      // BYOK users bring their own API keys, but must be signed in to BoxPlayer.
      if (isLoggedIn()) return { allowed: true }
      return { allowed: false, message: '请先登录 BoxPlayer 账号后使用 BYOK 模型' }
    }
    try { localStorage.setItem('boxplayer_show_pricing', '1') } catch {}
    return { allowed: false, message: cfg.vipOnlyMessage }
  }

  // VIP-only feature, user not VIP
  if (cfg.freeLimit === 0 && !vip) return { allowed: false, message: cfg.vipOnlyMessage }
  if (!metered) return { allowed: true }

  const effectiveLimit = feature === 'panHubSearch' && pro ? null : cfg.requiresPro ? cfg.vipLimit : vip ? cfg.vipLimit : cfg.freeLimit
  if (effectiveLimit === null) { incrementUsage(feature, amount); return { allowed: true } }

  const current = getUsageCount(feature)
  if (current + amount > effectiveLimit) {
    try { localStorage.setItem('boxplayer_show_pricing', '1') } catch {}
    return { allowed: false, message: vip ? cfg.vipMessage || cfg.freeMessage : cfg.freeMessage }
  }

  incrementUsage(feature, amount)
  return { allowed: true }
}
