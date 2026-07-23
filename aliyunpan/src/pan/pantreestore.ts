import { defineStore } from 'pinia'
import { IAliGetDirModel } from '../aliapi/alimodels'
import { h } from 'vue'
import IconFont from '../components/IconFont.vue'
import PanDAL from './pandal'
import TreeStore, { TreeNodeData } from '../store/treestore'
import { GetDriveID, isBaiduUser, isBoxUser, isCloud123User, isCloud139User, isCloud189User, isDrive115User, isDropboxUser, isGuangyaUser, isOneDriveUser, isPikPakUser, isQuarkUser } from '../aliapi/utils'
import message from '../utils/message'
import type { QuickFileEntry } from './quickFiles'
import { t } from '../i18n'

export interface PanTreeState {
  user_id: string
  drive_id: string
  default_drive_id: string
  backup_drive_id: string
  resource_drive_id: string
  pic_drive_id: string

  History: IAliGetDirModel[]

  selectDir: IAliGetDirModel

  selectDirPath: IAliGetDirModel[]

  treeData: TreeNodeData[]

  treeExpandedKeys: string[]

  treeSelectedKeys: string[]

  quickData: TreeNodeData[]

  scrollToDir: string
}

let treeDataMap = new Map<string, TreeNodeData>()
type State = PanTreeState

export const fileiconfn = (icon: string) => h(IconFont, { name: icon })

const usePanTreeStore = defineStore('pantree', {
  state: (): State => ({
    user_id: '',
    drive_id: '',
    default_drive_id: '',
    backup_drive_id: '',
    resource_drive_id: '',
    pic_drive_id: '',
    History: [],
    selectDir: {
      __v_skip: true,
      drive_id: '',
      file_id: '',
      album_id: '',
      parent_file_id: '',
      name: '',
      namesearch: '',
      size: 0,
      time: 0,
      description: ''
    },
    selectDirPath: [],
    treeData: [
      {
        __v_skip: true,
        title: t('pan.favorite'),
        namesearch: '',
        key: 'favorite',
        icon: () => fileiconfn('iconcrown'),
        isLeaf: true,
        children: []
      },
      {
        __v_skip: true,
        title: t('pan.screeningRoom'),
        namesearch: '',
        key: 'video',
        icon: () => fileiconfn('iconrss_video'),
        isLeaf: true,
        children: []
      },
      {
        __v_skip: true,
        title: t('pan.trash'),
        namesearch: '',
        key: 'trash',
        icon: () => fileiconfn('icondelete'),
        isLeaf: true,
        children: []
      },
      {
        __v_skip: true,
        title: t('pan.recover'),
        namesearch: '',
        key: 'recover',
        icon: () => fileiconfn('iconrecover'),
        isLeaf: true,
        children: []
      },
      {
        __v_skip: true,
        title: t('pan.globalSearch'),
        namesearch: '',
        key: 'search',
        icon: () => fileiconfn('iconsearch'),
        isLeaf: true,
        children: []
      },
      {
        __v_skip: true,
        title: t('pan.albumManagement'),
        namesearch: '',
        key: 'pic_root',
        icon: () => fileiconfn('iconjietu'),
        isLeaf: true,
        children: []
      },
      { __v_skip: true, title: t('drive.backup'), namesearch: '', key: 'backup_root', children: [] },
      { __v_skip: true, title: t('drive.resource'), namesearch: '', key: 'resource_root', children: [] }
    ],
    treeExpandedKeys: [],
    treeSelectedKeys: [],
    quickData: [],
    scrollToDir: ''
  }),
  getters: {
    PanHistoryCount(state: State): number {
      return state.History.length
    }
  },
  actions: {
    mTreeSelected(e: any, kuaijie: boolean = false) {
      let { key, drive_id = undefined } = e.node
      let is_refresh_drive_id = !['favorite', 'trash', 'recover'].includes(key) || !/color.*/g.test(key)
      const isCloudUser = isCloud123User(this.user_id || '')
      const isDrive115 = isDrive115User(this.user_id || '')
      const isBaidu = isBaiduUser(this.user_id || '')
      const isPikPak = isPikPakUser(this.user_id || '')
      const isDropbox = isDropboxUser(this.user_id || '')
      const isOneDrive = isOneDriveUser(this.user_id || '')
      const isBox = isBoxUser(this.user_id || '')
      const isQuark = isQuarkUser(this.user_id || '')
      const isCloud139 = isCloud139User(this.user_id || '')
      const isCloud189 = isCloud189User(this.user_id || '')
      const isGuangya = isGuangyaUser(this.user_id || '')
      if (isCloudUser) {
        const unsupported = ['video', 'recover', 'pic_root', 'backup_root', 'resource_root', 'favorite']
        if (unsupported.includes(key)) {
          message.info(t('pan.unsupportedFeature', { provider: t('drive.cloud123') }))
          return
        }
      }
      if ((isDrive115 || isBaidu) && key === 'resource_root') {
        message.info(t('pan.unsupportedFeature', { provider: isDrive115 ? t('drive.drive115') : t('drive.baiduFull') }))
        return
      }
      if (isPikPak && ['video', 'recover', 'pic_root', 'backup_root', 'resource_root', 'favorite'].includes(key)) {
        message.info(t('pan.unsupportedFeature', { provider: 'PikPak' }))
        return
      }
      if (isQuark && ['video', 'recover', 'pic_root', 'backup_root', 'resource_root', 'favorite'].includes(key)) {
        message.info(t('pan.unsupportedFeature', { provider: t('drive.quarkFull') }))
        return
      }
      if ((isCloud139 || isCloud189 || isGuangya) && ['video', 'recover', 'pic_root', 'backup_root', 'resource_root', 'favorite', 'trash'].includes(key)) {
        message.info(t('pan.unsupportedFeature', { provider: isCloud139 ? t('drive.cloud139') : isCloud189 ? t('drive.cloud189') : t('drive.guangya') }))
        return
      }
      if (isDropbox && ['video', 'recover', 'pic_root', 'backup_root', 'resource_root', 'favorite', 'trash'].includes(key)) {
        message.info(t('pan.unsupportedFeature', { provider: 'Dropbox' }))
        return
      }
      if (isOneDrive && ['video', 'recover', 'pic_root', 'backup_root', 'resource_root', 'favorite'].includes(key)) {
        message.info(t('pan.unsupportedFeature', { provider: 'OneDrive' }))
        return
      }
      if (isBox && ['video', 'recover', 'pic_root', 'backup_root', 'resource_root', 'favorite'].includes(key)) {
        message.info(t('pan.unsupportedFeature', { provider: 'Box' }))
        return
      }
      if (!kuaijie) {
        const getParentNode = (node: any): any => {
          return node.parent ? getParentNode(node.parent) : node
        }
        const parentNode = getParentNode(e.node)
        drive_id = GetDriveID(this.user_id, parentNode.key || key)
      }
      console.log('mTreeSelected', e, drive_id)
      if (is_refresh_drive_id && drive_id) {
        this.drive_id = drive_id
      }
      if (key === 'video') this.drive_id = GetDriveID(this.user_id, 'backup')
      if (key === 'pic_root') key = this.selectDir.album_type || 'pic_root'
      else this.selectDir.album_id = ''
      PanDAL.aReLoadOneDirToShow('', key, true)
    },
    mTreeExpand(key: string) {
      const arr = this.treeExpandedKeys
      if (arr.includes(key)) {
        const dirPath = TreeStore.GetDirPath(this.drive_id, this.selectDir.file_id)
        const needSelectNew = dirPath.filter((t) => t.parent_file_id == key).length > 0
        this.treeExpandedKeys = arr.filter((t) => t != key)
        if (needSelectNew) PanDAL.aReLoadOneDirToShow('', key, false)
      } else {
        this.treeExpandedKeys = arr.concat([key])
        PanDAL.RefreshPanTreeAllNode(this.drive_id)
      }
      console.log('mTreeExpand.treeExpandedKeys', this.treeExpandedKeys)
    },

    mTreeExpandAll(keyList: string[], isExpaned: boolean) {
      const arr = new Set(this.treeExpandedKeys)
      if (isExpaned) {
        for (let i = 0, maxi = keyList.length; i < maxi; i++) {
          arr.add(keyList[i])
        }
      } else {
        for (let i = 0, maxi = keyList.length; i < maxi; i++) {
          arr.delete(keyList[i])
        }
      }
      this.treeExpandedKeys = Array.from(arr)
      if (isExpaned) PanDAL.RefreshPanTreeAllNode(this.drive_id)
    },

    mSaveUser(user_id: string, default_drive_id: string, resource_drive_id: string, backup_drive_id: string, pic_drive_id: string) {
      this.$reset()
      this.$patch({ user_id, default_drive_id, resource_drive_id, backup_drive_id, pic_drive_id })
    },

    mShowDir(dir: IAliGetDirModel, dirPath: IAliGetDirModel[], treeSelectedKeys: string[], treeExpandedKeys: string[]) {
      this.$patch({
        selectDir: dir,
        selectDirPath: dirPath,
        treeSelectedKeys: treeSelectedKeys,
        treeExpandedKeys: treeExpandedKeys
      })
    },

    mSaveTreeAllNode(drive_id: string, root: TreeNodeData, rootMap: Map<string, TreeNodeData>) {
      if (this.drive_id !== drive_id) return
      const list: TreeNodeData[] = []
      let hasRoot = false
      for (let i = 0, maxi = this.treeData.length; i < maxi; i++) {
        if (this.treeData[i].key == root.key) {
          list.push(root)
          hasRoot = true
        } else list.push(this.treeData[i])
      }
      if (!hasRoot) list.push(root)
      this.treeData = list
      treeDataMap = rootMap
    },

    mRenameFiles(fileList: { file_id: string; parent_file_id: string; name: string; isDir: boolean }[]) {
      let isChange = false
      let isPath = false

      const diridList: string[] = []
      for (let i = 0, maxi = fileList.length; i < maxi; i++) {
        const item = fileList[i]
        if (!item.isDir) continue
        diridList.push(item.file_id)
        const findNode = treeDataMap.get(item.file_id)
        if (findNode) {
          findNode.title = item.name
          isChange = true
        }
        if (this.selectDir.file_id == item.file_id) {
          this.selectDir = Object.assign({}, this.selectDir, { name: item.name }) as IAliGetDirModel
          isChange = true
        }

        this.selectDirPath.map((t) => {
          if (t.file_id == item.file_id) {
            t.name = item.name
            isPath = true
          }
          return true
        })
      }

      if (isChange) this.treeData = this.treeData.concat()
      if (isPath) this.selectDirPath = this.selectDirPath.concat()


      TreeStore.RenameDirs(this.drive_id, fileList)
    },
    mSaveQuick(list: QuickFileEntry[]) {
      const nodeList: TreeNodeData[] = []
      for (let i = 0; i < list.length; i++) {
        nodeList.push({
          __v_skip: true,
          key: list[i].id,
          shortcut_id: list[i].id,
          user_id: list[i].user_id,
          user_name: list[i].user_name,
          provider: list[i].provider,
          drive_id: list[i].drive_id,
          drive_name: list[i].drive_name,
          file_id: list[i].file_id,
          parent_file_id: list[i].parent_file_id,
          path: list[i].path,
          description: list[i].description,
          dir_path: list[i].dir_path,
          title: list[i].title || list[i].file_id,
          namesearch: i < 9 ? t('pan.shortcut', { number: i + 1 }) : '',
          children: [],
          isLeaf: true
        } as TreeNodeData)
      }
      Object.freeze(nodeList)
      this.quickData = nodeList
    },
    mSaveTreeScrollTo(dirID: string) {
      if (dirID == 'refresh') dirID = this.selectDir.file_id
      this.scrollToDir = dirID
    }
  }
})

export default usePanTreeStore
