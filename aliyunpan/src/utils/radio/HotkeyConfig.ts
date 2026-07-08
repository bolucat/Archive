export interface MineradioHotkey {
  id: string
  label: string
  value: string
}

export const DEFAULT_MINERADIO_HOTKEYS: MineradioHotkey[] = [
  { id: 'toggle', label: '播放 / 暂停', value: 'Space' },
  { id: 'prev', label: '上一首', value: 'Shift+ArrowLeft' },
  { id: 'next', label: '下一首', value: 'Shift+ArrowRight' },
  { id: 'seekBack', label: '快退 5 秒', value: 'ArrowLeft' },
  { id: 'seekForward', label: '快进 5 秒', value: 'ArrowRight' },
  { id: 'volUp', label: '音量增加', value: 'ArrowUp' },
  { id: 'volDown', label: '音量降低', value: 'ArrowDown' },
  { id: 'immersiveExit', label: '退出沉浸模式', value: 'Escape' }
]

export function normalizeHotkeyValue(value: string) {
  return value.split('+').map((part) => part.trim()).filter(Boolean).join('+')
}

export function hotkeyFromEvent(e: KeyboardEvent) {
  const parts: string[] = []
  if (e.ctrlKey) parts.push('Ctrl')
  if (e.altKey) parts.push('Alt')
  if (e.shiftKey) parts.push('Shift')
  if (e.metaKey) parts.push('Meta')
  const key = e.key === ' ' ? 'Space' : e.key.length === 1 ? e.key.toUpperCase() : e.key
  if (!['Control', 'Alt', 'Shift', 'Meta'].includes(key)) parts.push(key)
  return parts.join('+')
}

export function hasHotkeyConflict(items: MineradioHotkey[], item: MineradioHotkey) {
  const value = normalizeHotkeyValue(item.value)
  return !!value && items.some((other) => other.id !== item.id && normalizeHotkeyValue(other.value) === value)
}

export function loadHotkeys(): MineradioHotkey[] {
  try {
    const raw = JSON.parse(localStorage.getItem('pm.hotkeys') || '[]')
    const byId = new Map(Array.isArray(raw) ? raw.map((item: MineradioHotkey) => [item.id, item]) : [])
    return DEFAULT_MINERADIO_HOTKEYS.map((item) => ({ ...item, value: normalizeHotkeyValue(String((byId.get(item.id) as MineradioHotkey | undefined)?.value || item.value)) }))
  } catch {
    return DEFAULT_MINERADIO_HOTKEYS.map((item) => ({ ...item }))
  }
}

export function saveHotkeys(items: MineradioHotkey[]) {
  localStorage.setItem('pm.hotkeys', JSON.stringify(items.map((item) => ({ ...item, value: normalizeHotkeyValue(item.value) }))))
}
