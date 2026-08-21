import posthog from 'posthog-js'
import axios from '../axios'
import { POSTHOG_HOST, POSTHOG_PROJECT_API_KEY } from '../secrets.generated'
import { useSettingStore } from '../store'

declare const __APP_VERSION__: string

type Provider = 'aliyun' | 'cloud123' | '115' | '139' | '189' | 'guangya' | 'baidu' | 'pikpak' | 'quark' | 'dropbox' | 'onedrive' | 'box' | 'google' | 'webdav' | 'alist'

const installIdKey = 'boxplayer.analytics.install_id'
let initialized = false
let networkFailureTrackingInstalled = false

type CloudApiFailure = { provider: Provider; statusCode?: number; failureKind: 'http' | 'network'; requestUrl: string; serverError?: string }

const secretKeyPattern = 'access[_-]?token|refresh[_-]?token|id[_-]?token|authorization|cookie|password|client[_-]?secret|signature|x-signature|share[_-]?token'
const isSecretKey = (key: string) => new RegExp(`^(?:${secretKeyPattern})$`, 'i').test(key)
const redactSecretFields = (text: string) => text
  .replace(new RegExp(`(["'](?:${secretKeyPattern})["']\s*:\s*["'])[^"']*`, 'gi'), '$1[REDACTED]')
  .replace(new RegExp(`(\b(?:${secretKeyPattern})\b\s*=\s*)[^,\r\n;\s]+`, 'gi'), '$1[REDACTED]')

export const redactAnalyticsSecrets = (value: unknown): string => {
  const text = typeof value === 'string' ? value : (() => {
    try { return JSON.stringify(value) } catch { return String(value || '') }
  })()
  const redactedUrl = text.replace(/https?:\/\/[^\s"']+/g, (url) => {
    try {
      const parsed = new URL(url)
      parsed.searchParams.forEach((_value, key) => {
        if (isSecretKey(key)) parsed.searchParams.set(key, '[REDACTED]')
      })
      return parsed.toString().replace(/%5BREDACTED%5D/gi, '[REDACTED]')
    } catch {
      return url
    }
  })
  const redactedHeaders = redactedUrl
    .replace(/(authorization\s*[:=]\s*bearer\s+)[^,\r\n;]+/gi, '$1[REDACTED]')
    .replace(/(authorization\s*[:=]\s*)(?!bearer\s+)[^,\r\n;]+/gi, '$1[REDACTED]')
    .replace(/(cookie\s*[:=]\s*)[^\r\n]+/gi, '$1[REDACTED]')
    .replace(/(password\s*[:=]\s*)[^,\r\n;]+/gi, '$1[REDACTED]')
  return redactSecretFields(redactedHeaders)
}

export const resolveCloudApiFailure = (url: string, statusCode?: number, serverError?: unknown): CloudApiFailure | undefined => {
  const host = (() => {
    try { return new URL(url).hostname } catch { return '' }
  })()
  const provider =
    /(^|\.)aliyundrive\.com$|(^|\.)alipan\.com$/.test(host) ? 'aliyun' :
      /(^|\.)123pan\.com$/.test(host) ? 'cloud123' :
        /(^|\.)115\.com$/.test(host) ? '115' :
          /(^|\.)baidu\.com$/.test(host) ? 'baidu' :
            /(^|\.)mypikpak\.com$/.test(host) ? 'pikpak' :
              /(^|\.)quark\.cn$/.test(host) ? 'quark' :
                /(^|\.)139\.com$/.test(host) ? '139' :
                  /(^|\.)189\.cn$/.test(host) ? '189' :
                    /(^|\.)dropboxapi\.com$/.test(host) ? 'dropbox' :
                      /(^|\.)microsoft\.com$/.test(host) ? 'onedrive' :
                        /(^|\.)box\.com$/.test(host) ? 'box' :
                          /(^|\.)googleapis\.com$/.test(host) ? 'google' : undefined
  return provider ? { provider, statusCode, failureKind: statusCode ? 'http' : 'network', requestUrl: redactAnalyticsSecrets(url), serverError: serverError === undefined ? undefined : redactAnalyticsSecrets(serverError) } : undefined
}

export const shouldCaptureCloudApiFailure = (failure: CloudApiFailure): boolean => {
  // AliHttp refreshes an expired Aliyun token and retries the original request.
  // The first 401 is expected in that recovery path, not a user-visible failure.
  return !(failure.provider === 'aliyun' && failure.statusCode === 401)
}

const getInstallId = () => {
  let id = localStorage.getItem(installIdKey)
  if (!id) {
    id = crypto.randomUUID()
    localStorage.setItem(installIdKey, id)
  }
  return id
}

export const isAnalyticsConfigured = () => Boolean(POSTHOG_PROJECT_API_KEY && POSTHOG_HOST)

export const startAnalytics = () => {
  if (initialized || !isAnalyticsConfigured()) return
  posthog.init(POSTHOG_PROJECT_API_KEY, {
    api_host: POSTHOG_HOST,
    autocapture: false,
    capture_pageview: false,
    capture_pageleave: false,
    disable_session_recording: true,
    persistence: 'localStorage',
    loaded: (client) => {
      client.identify(getInstallId())
      client.capture('app_opened', { app_version: __APP_VERSION__, platform: window.platform || process.platform, locale: useSettingStore().uiLanguage })
    }
  })
  initialized = true
  installCloudApiFailureTracking()
}

export const captureProviderLogin = (provider: Provider) => {
  if (!initialized) return
  posthog.capture('provider_login_succeeded', { provider })
}

export const captureCloudApiFailure = (failure: CloudApiFailure) => {
  if (!initialized) return
  posthog.capture('cloud_api_request_failed', {
    provider: failure.provider,
    failure_kind: failure.failureKind,
    status_code: failure.statusCode || 0,
    request_url: failure.requestUrl,
    server_error: failure.serverError || ''
  })
}

const installCloudApiFailureTracking = () => {
  if (networkFailureTrackingInstalled) return
  networkFailureTrackingInstalled = true
  const originalFetch = window.fetch.bind(window)
  window.fetch = async (input, init) => {
    const url = typeof input === 'string' ? input : input instanceof Request ? input.url : input.toString()
    try {
      const response = await originalFetch(input, init)
      if (!response.ok) {
        const responseBody = await response.clone().text().catch(() => '')
        const failure = resolveCloudApiFailure(url, response.status, responseBody)
        if (failure && shouldCaptureCloudApiFailure(failure)) captureCloudApiFailure(failure)
      }
      return response
    } catch (error) {
      const failure = resolveCloudApiFailure(url, undefined, error instanceof Error ? error.message : error)
      if (failure && shouldCaptureCloudApiFailure(failure)) captureCloudApiFailure(failure)
      throw error
    }
  }
  axios.interceptors.response.use(undefined, (error) => {
    const failure = resolveCloudApiFailure(String(error?.config?.url || ''), Number(error?.response?.status) || undefined, error?.response?.data || error?.message)
    if (failure && shouldCaptureCloudApiFailure(failure)) captureCloudApiFailure(failure)
    return Promise.reject(error)
  })
}
