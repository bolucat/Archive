import { videoChange } from '@/core/observer'
import { matchUrlPattern } from '@/core/utils'
import { playerUrls } from '@/core/utils/urls'

export interface VideoControlBarItem {
  name: string
  displayName: string
  icon: string
  order: number
  action: (event: MouseEvent) => void | Promise<void>
}
const controlBarClass = '.be-video-control-bar-extend'
type ControlBarComponent = { items: VideoControlBarItem[] }
let controlBarInstance: Promise<unknown> | null = null

const initControlBar = lodash.once(() => {
  if (!playerUrls.some(url => matchUrlPattern(url))) {
    return Promise.resolve<unknown>(null)
  }
  return new Promise<unknown>(resolve => {
    videoChange(async () => {
      const { playerAgent } = await import('@/components/video/player-agent')
      const time = await playerAgent.query.control.buttons.time()
      const VideoControlBar = await import('./VideoControlBar.vue').then(m => m.default)
      if (time === null || time.parentElement?.querySelector(controlBarClass) !== null) {
        return
      }
      const instance = new VideoControlBar().$mount()
      time.insertAdjacentElement('afterend', instance.$el)
      resolve(instance)
    })
  })
})
/** 向视频控制栏添加按钮 */
export const addControlBarButton = async (button: VideoControlBarItem) => {
  if (!controlBarInstance) {
    controlBarInstance = initControlBar()
  }
  const created = (await controlBarInstance) as ControlBarComponent
  if (!created) {
    return
  }
  created.items.push(button)
}
