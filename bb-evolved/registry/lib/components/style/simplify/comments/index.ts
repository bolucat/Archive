import { wrapSwitchOptions } from '@/components/switch-options'

const name = 'simplifyComments'
export const component = wrapSwitchOptions({
  name: 'simplifyOptions',
  switchProps: {
    checkedIcon: 'mdi-checkbox-marked-circle',
    notCheckedIcon: 'mdi-checkbox-blank-circle-outline',
  },
  dimAt: false,
  switches: {
    userLevel: {
      defaultValue: true,
      displayName: '用户等级',
    },
    decorateAndTime: {
      defaultValue: true,
      displayName: '装扮 & 时间',
    },
    userPendent: {
      defaultValue: false,
      displayName: '头像框',
    },
    subReplyNewLine: {
      defaultValue: true,
      displayName: '回复换行',
    },
    replyEditor: {
      defaultValue: true,
      displayName: '编辑框',
    },
    fansMedal: {
      defaultValue: false,
      displayName: '粉丝勋章',
    },
    eventBanner: {
      defaultValue: true,
      displayName: '小喇叭横幅',
    },
  },
})({
  name,
  displayName: '简化评论区',
  entry: async ({ metadata }) => {
    const { addComponentListener } = await import('@/core/settings')
    addComponentListener(
      metadata.name,
      (value: boolean) => {
        document.body.classList.toggle('simplify-comment', value)
      },
      true,
    )

    const { ShadowDomStyles } = await import('@/core/shadow-dom')
    const v3Styles = await import('./comments-v3.scss').then(m => m.default)
    const shadowDom = new ShadowDomStyles()
    shadowDom.addStyle(v3Styles)
  },
  instantStyles: [
    {
      name: `${name}v1`,
      style: () => import('./comments.scss'),
    },
    {
      name: `${name}v2`,
      style: () => import('./comments-v2.scss'),
    },
  ],
  tags: [componentsTags.style],
})
