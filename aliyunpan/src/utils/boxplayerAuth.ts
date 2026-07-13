import { createClient, type SupabaseClient } from '@supabase/supabase-js'
import { BOXPLAYER_SITE_URL as GENERATED_BOXPLAYER_SITE_URL, BOXPLAYER_SUPABASE_URL, SUPABASE_PUBLISHABLE_KEY } from '../secrets.generated'

export const BOXPLAYER_SITE_URL = GENERATED_BOXPLAYER_SITE_URL

let supabaseClient: SupabaseClient | null | undefined

export function getBoxPlayerSupabase(): SupabaseClient | null {
  if (supabaseClient !== undefined) return supabaseClient
  supabaseClient = BOXPLAYER_SUPABASE_URL && SUPABASE_PUBLISHABLE_KEY ? createClient(BOXPLAYER_SUPABASE_URL, SUPABASE_PUBLISHABLE_KEY) : null
  return supabaseClient
}

export async function getBoxPlayerAccessToken(forceRefresh = false): Promise<string> {
  const supabase = getBoxPlayerSupabase()
  if (!supabase) throw new Error('登录服务未配置')
  if (forceRefresh) {
    const { data: refreshed } = await supabase.auth.refreshSession()
    const refreshedToken = refreshed.session?.access_token
    if (refreshedToken) return refreshedToken
  }
  const { data } = await supabase.auth.getSession()
  const token = data.session?.access_token
  if (!token) throw new Error('请先登录 BoxPlayer 账号')
  return token
}

export async function fetchBoxPlayerSubscription(): Promise<any> {
  let token = await getBoxPlayerAccessToken()
  let response = await fetch(`${BOXPLAYER_SITE_URL}/api/me/subscription`, { headers: { Authorization: `Bearer ${token}` } })
  let data = await response.json().catch(() => ({}))
  if (!response.ok && data.error === 'invalid_auth_token') {
    token = await getBoxPlayerAccessToken(true)
    response = await fetch(`${BOXPLAYER_SITE_URL}/api/me/subscription`, { headers: { Authorization: `Bearer ${token}` } })
    data = await response.json().catch(() => ({}))
  }
  if (!response.ok) throw new Error(data.error || 'subscription_lookup_failed')
  return data
}
