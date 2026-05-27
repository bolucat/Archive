import { Modal } from '@arco-design/web-vue'
import { h } from 'vue'
import useSettingStore from '../setting/settingstore'
import { triggerMusicScanIfDue } from './musicLibraryBootstrap'
import { triggerMediaScanIfDue } from './mediaLibraryBootstrap'
import DebugLog from './debuglog'

let inflightPrompt: Promise<void> | null = null

export async function promptAutoScanForUser(userId: string, userLabel: string): Promise<void> {
  if (!userId) return
  const setting = useSettingStore()
  const list = Array.isArray(setting.uiLibraryAutoScanPromptedUsers) ? setting.uiLibraryAutoScanPromptedUsers : []
  if (list.includes(userId)) return
  // 串行化弹窗，避免多个账号同时登录时叠满屏
  while (inflightPrompt) {
    try { await inflightPrompt } catch { /* ignore */ }
  }
  inflightPrompt = (async () => {
    try {
      const choice = await askUser(userLabel)
      const next = Array.from(new Set([...list, userId]))
      const partial: Record<string, unknown> = { uiLibraryAutoScanPromptedUsers: next }
      // 全局开关：只要勾选一项，开启对应全局开关；不勾不动现有总开关
      if (choice.music) partial.uiLibraryAutoScanMusic = true
      if (choice.video) partial.uiLibraryAutoScanVideo = true
      // per-user 启用/禁用：未勾的把 user_id 加入 disabled 列表
      const musicDisabled = new Set(setting.uiLibraryAutoScanMusicDisabledUsers || [])
      const videoDisabled = new Set(setting.uiLibraryAutoScanVideoDisabledUsers || [])
      if (choice.music) musicDisabled.delete(userId)
      else musicDisabled.add(userId)
      if (choice.video) videoDisabled.delete(userId)
      else videoDisabled.add(userId)
      partial.uiLibraryAutoScanMusicDisabledUsers = Array.from(musicDisabled)
      partial.uiLibraryAutoScanVideoDisabledUsers = Array.from(videoDisabled)
      await setting.updateStore(partial)
      if (choice.music) {
        triggerMusicScanIfDue(true).catch((e) => DebugLog.mSaveWarning('post-login music scan: ' + (e as Error).message))
      }
      if (choice.video) {
        triggerMediaScanIfDue(true).catch((e) => DebugLog.mSaveWarning('post-login video scan: ' + (e as Error).message))
      }
    } catch (e) {
      DebugLog.mSaveWarning('promptAutoScanForUser failed: ' + (e as Error).message)
    } finally {
      inflightPrompt = null
    }
  })()
  return inflightPrompt
}

interface ScanChoice {
  music: boolean
  video: boolean
}

function askUser(userLabel: string): Promise<ScanChoice> {
  return new Promise<ScanChoice>((resolve) => {
    const setting = useSettingStore()
    const choice: ScanChoice = {
      music: setting.uiLibraryAutoScanMusic === true,
      video: setting.uiLibraryAutoScanVideo === true
    }
    Modal.confirm({
      title: '开启后台静默扫描？',
      okText: '保存设置',
      cancelText: '暂不开启',
      simple: false,
      maskClosable: false,
      content: () => h('div', { style: 'min-width: 360px; line-height: 1.7' }, [
        h('div', { style: 'color: var(--color-text-2); font-size: 13px; margin-bottom: 10px' }, [
          `检测到新账号「${userLabel}」登录成功，是否开启后台静默扫描以收录该账号下的音乐与视频？`
        ]),
        h('div', { style: 'display: flex; flex-direction: column; gap: 8px' }, [
          h('label', { style: 'cursor: pointer; user-select: none' }, [
            h('input', {
              type: 'checkbox',
              checked: choice.music,
              style: 'margin-right: 6px; vertical-align: middle',
              onChange: (e: Event) => {
                choice.music = (e.target as HTMLInputElement).checked
              }
            }),
            '音乐库：每次启动 App 静默扫描音频文件'
          ]),
          h('label', { style: 'cursor: pointer; user-select: none' }, [
            h('input', {
              type: 'checkbox',
              checked: choice.video,
              style: 'margin-right: 6px; vertical-align: middle',
              onChange: (e: Event) => {
                choice.video = (e.target as HTMLInputElement).checked
              }
            }),
            '视频媒体库：每次启动 App 重扫已加入媒体库的文件夹'
          ]),
          h('div', { style: 'color: var(--color-text-3); font-size: 12px; margin-top: 4px' },
            '稍后可在「设置 → 网盘 → 媒体库后台自动扫描」中随时调整。')
        ])
      ]),
      onOk: () => resolve({ music: choice.music, video: choice.video }),
      onCancel: () => resolve({ music: false, video: false })
    })
  })
}
