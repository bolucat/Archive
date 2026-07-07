const MUSIC_PLAYER_DBLCLICK_IGNORE_SELECTOR = [
  '.amp-bottom',
  '.amp-titlebar',
  '.amp-drawer',
  '.amp-icon-btn',
  '.amp-ctrl-btn',
  '.amp-play-btn',
  '.amp-bar',
  '.amp-sleep-menu',
  'button',
  'input',
  'textarea',
  'select',
  'a'
].join(',')

export function shouldHandleMusicPlayerDblClick(event: MouseEvent) {
  const target = event.target
  if (!target || typeof (target as Element).closest !== 'function') return true
  return !(target as Element).closest(MUSIC_PLAYER_DBLCLICK_IGNORE_SELECTOR)
}
