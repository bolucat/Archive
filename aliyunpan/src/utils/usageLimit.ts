export type FeatureName = 'aiAgentChat' | 'panHubSave' | 'readerAIChat' | 'readerTTS' | 'readerTranslation'

export interface FeatureConfig { freeLimit: number; vipLimit: number | null; unit: string; freeMessage: string; vipMessage: string; vipOnlyMessage: string }
export interface UsageCheckResult { allowed: boolean; message?: string }

const CONFIGS: Record<FeatureName, FeatureConfig> = {
  aiAgentChat:    { freeLimit: 5, vipLimit: null, unit: 'sessions',  freeMessage: '今日AI对话次数已用完（5次），登录后可无限使用', vipMessage: '', vipOnlyMessage: '' },
  panHubSave:     { freeLimit: 5, vipLimit: null, unit: 'saves',    freeMessage: '今日保存次数已用完（5次），登录后可无限使用', vipMessage: '', vipOnlyMessage: '' },
  readerAIChat:   { freeLimit: 0, vipLimit: 5, unit: 'sessions',   freeMessage: '', vipMessage: '今日AI对话次数已用完（5次），明日重置', vipOnlyMessage: 'AI对话需登录后使用' },
  readerTTS:      { freeLimit: 0, vipLimit: 5000, unit: 'chars',   freeMessage: '', vipMessage: '今日朗读字符数已用完（5000字），明日重置', vipOnlyMessage: '朗读功能需登录后使用' },
  readerTranslation: { freeLimit: 0, vipLimit: 5000, unit: 'chars', freeMessage: '', vipMessage: '今日翻译字符数已用完（5000字），明日重置', vipOnlyMessage: '翻译功能需登录后使用' },
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

export function getUsageCount(feature: FeatureName): number {
  try { const v = parseInt(localStorage.getItem(getKey(feature)) || '0', 10); return Number.isFinite(v) ? v : 0 } catch { return 0 }
}

export function incrementUsage(feature: FeatureName, amount = 1): void {
  try { localStorage.setItem(getKey(feature), String(getUsageCount(feature) + amount)) } catch {}
}

export function getRemainingCredits(feature: FeatureName): number | 'unlimited' {
  const cfg = CONFIGS[feature]
  if (!isVIP() && cfg.freeLimit === 0) return 0
  const effectiveLimit = isVIP() ? cfg.vipLimit : cfg.freeLimit
  if (effectiveLimit === null) return 'unlimited'
  return Math.max(0, effectiveLimit - getUsageCount(feature))
}

export function checkAndIncrement(feature: FeatureName, amount = 1): UsageCheckResult {
  cleanOldKeys()
  const cfg = CONFIGS[feature]
  const vip = isVIP()

  // VIP-only feature, user not VIP
  if (cfg.freeLimit === 0 && !vip) return { allowed: false, message: cfg.vipOnlyMessage }

  const effectiveLimit = vip ? cfg.vipLimit : cfg.freeLimit
  if (effectiveLimit === null) { incrementUsage(feature, amount); return { allowed: true } }

  const current = getUsageCount(feature)
  if (current + amount > effectiveLimit) {
    try { localStorage.setItem('boxplayer_show_pricing', '1') } catch {}
    return { allowed: false, message: vip ? cfg.vipMessage || cfg.freeMessage : cfg.freeMessage }
  }

  incrementUsage(feature, amount)
  return { allowed: true }
}
