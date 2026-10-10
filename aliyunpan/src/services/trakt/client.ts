import { ref } from 'vue'
import type { TraktIdentity, TraktStatus, TraktAccount } from '@shared/trakt'
import { t } from '../../i18n'

export const traktStatus = ref<TraktStatus>({ configured: false, connected: false })
export const traktAccount = ref<TraktAccount>()
function readSyncEnabled() { try { return typeof window === 'undefined' || window.localStorage.getItem('trakt-sync-enabled') !== 'false' } catch { return true } }
export const traktSyncEnabled = ref(readSyncEnabled())
export function setTraktSyncEnabled(enabled: boolean) { localStorage.setItem('trakt-sync-enabled', String(enabled)); traktSyncEnabled.value = enabled }
function invoke<T>(operation: string, ...args: unknown[]): Promise<T> {
  const ipc = window.Electron?.ipcRenderer
  if (!ipc) return Promise.reject(new Error('TRAKT_DESKTOP'))
  return ipc.invoke('trakt:' + operation, ...args) as Promise<T>
}
export async function loadTraktStatus() { traktStatus.value = await invoke<TraktStatus>('status'); return traktStatus.value }
export async function loadTraktAccount() { const account = await invoke<TraktAccount>('account'); if (traktStatus.value.connected) traktAccount.value = account; return account }
export const startTraktLogin = () => invoke<TraktStatus>('start')
export const openTraktVip = () => invoke<void>('vip')
export const cancelTraktLogin = () => invoke<void>('cancel')
export async function logoutTrakt() { traktStatus.value = await invoke<TraktStatus>('logout'); traktAccount.value = undefined }
export function syncTrakt(identity: TraktIdentity, action: 'watchlist-add' | 'watchlist-remove' | 'rating', rating?: number) {
  if (!traktSyncEnabled.value) return Promise.resolve()
  return invoke<void>('sync', JSON.parse(JSON.stringify({ type: identity.type, tmdbId: identity.tmdbId, imdbId: identity.imdbId })), action, rating).catch(async error => {
    if (String(error).includes('TRAKT_LOGIN')) await loadTraktStatus().catch(() => undefined)
    throw error
  })
}
export function traktError(error: unknown) {
  const code = String(error).match(/TRAKT_[A-Z_]+/)?.[0]
  switch (code) {
    case 'TRAKT_CONFIG': return t('trakt.configRequired')
    case 'TRAKT_METADATA': case 'TRAKT_NOT_FOUND': return t('trakt.metadataRequired')
    case 'TRAKT_SECURE_STORAGE': return t('trakt.storageError')
    case 'TRAKT_EXPIRED': return t('trakt.expired')
    case 'TRAKT_DENIED': return t('trakt.denied')
    case 'TRAKT_LOGIN': return t('trakt.loginRequired')
    case 'TRAKT_FORBIDDEN': return t('trakt.forbidden')
    case 'TRAKT_STATS': return t('trakt.statsError')
    case 'TRAKT_RESPONSE': return t('trakt.statsError')
    case 'TRAKT_RATE_LIMIT': case 'TRAKT_LIMIT': return t('trakt.rateLimit')
    default: return t('trakt.failed')
  }
}
