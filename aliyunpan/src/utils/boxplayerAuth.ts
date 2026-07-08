import { createClient, type SupabaseClient } from '@supabase/supabase-js'

const SUPABASE_URL = 'https://ltqipofjjqjlbbfsgihi.supabase.co'
const SUPABASE_ANON_KEY = 'sb_publishable_VzoE4CzxiTaNpFVkFUc8cA_XARw0T3r'

let supabaseClient: SupabaseClient | null | undefined

export function getBoxPlayerSupabase(): SupabaseClient | null {
  if (supabaseClient !== undefined) return supabaseClient
  supabaseClient = SUPABASE_URL && SUPABASE_ANON_KEY ? createClient(SUPABASE_URL, SUPABASE_ANON_KEY) : null
  return supabaseClient
}

export async function getBoxPlayerAccessToken(): Promise<string> {
  const supabase = getBoxPlayerSupabase()
  if (!supabase) throw new Error('登录服务未配置')
  const { data } = await supabase.auth.getSession()
  const token = data.session?.access_token
  if (!token) throw new Error('请先登录 BoxPlayer 账号')
  return token
}
