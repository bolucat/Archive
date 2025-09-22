import DebugLog from '../utils/debuglog'
import { humanDateTime, humanExpiration } from '../utils/format'
import message from '../utils/message'
import AliHttp, { IUrlRespData } from './alihttp'
import { IAliShareItem } from './alimodels'
import AliDirFileList from './dirfilelist'

export interface IAliShareResp {
  items: IAliShareItem[]
  itemsKey: Set<string>
  next_marker: string
  m_time: number 
  m_user_id: string 
}

export interface IAliShareResp {
  items: IAliShareItem[]
  itemsKey: Set<string>
  next_marker: string
  m_time: number
  m_user_id: string
}

export default class AliTransferShareList {
  static async ApiTransferShareListAll(user_id: string): Promise<IAliShareResp> {
    const dir: IAliShareResp = {
      items: [],
      itemsKey: new Set(),
      next_marker: '',
      m_time: 0,
      m_user_id: user_id
    }
    await AliTransferShareList.ApiShareListOnePage(user_id, dir)
    return dir
  }
  static async ApiShareListOnePage(user_id: string, dir: IAliShareResp): Promise<boolean> {
    const url = 'adrive/v1/share/list'
    const postData = {
      limit: 20,
      order_by: 'created_at',
      order_direction: 'DESC'
    }
    const resp = await AliHttp.Post(url, postData,user_id,'')
    return await AliTransferShareList._TransferShareListOnePage(user_id, dir, resp)
  }

  static async _TransferShareListOnePage(user_id: string, dir: IAliShareResp, resp: IUrlRespData): Promise<boolean> {
    try {
      if (AliHttp.IsSuccess(resp.code)) {
        const timeNow = new Date().getTime()
        const downUrl = 'https://api.aliyundrive.com/v2/file/download?t=' + Date.now().toString()
        for (let i = 0, maxi = resp.body.items.length; i < maxi; i++) {
          const item = resp.body.items[i] as IAliShareItem
          if (dir.itemsKey.has(item.share_id)) continue
          let icon = 'iconwenjian'
          let first_file: any = item.share_id && await AliTransferShareList.ApiTransferShareFileStatus(user_id, item.share_id)
          if (first_file && first_file.drive_files) {
            let info = AliDirFileList.getFileInfo(dir.m_user_id, first_file.drive_files[0], downUrl)
            icon = info.icon || 'iconwenjian'
          }
          const add = Object.assign({}, item, { first_file, icon }) as IAliShareItem
          if (!add.full_share_msg) add.full_share_msg = ''
          if (!add.share_msg) add.share_msg = ''
          if (!add.share_name) add.share_name = 'share_name'
          if (!add.is_share_saved) add.share_saved = '未保存'
          else add.share_saved = '已保存'
          if (!add.expired) add.expired = false
          if (item.created_at) {
            add.created_at = humanDateTime(new Date(item.created_at).getTime())
          } else {
            add.created_at = ''
          }
          add.share_msg = humanExpiration(item.expiration, timeNow)
          if (item.status == 'forbidden') add.share_msg = '分享违规'
          dir.items.push(add)
          dir.itemsKey.add(add.share_id)
        }
        return true
      } else if (resp.code == 404) {
        dir.items.length = 0
        return true
      } else if (resp.body && resp.body.code) {
        dir.items.length = 0
        message.warning('列出分享列表出错' + resp.body.code, 2)
        return false
      } else if (!AliHttp.HttpCodeBreak(resp.code)) {
        DebugLog.mSaveWarning('_ShareListOnePage err=' + (resp.code || ''), resp.body)
      }
    } catch (err: any) {
      DebugLog.mSaveDanger('_ShareListOnePage', err)
    }
    return false
  }

  static async ApiTransferShareFileStatus(user_id: string, share_id: string): Promise<any> {
    const url = 'adrive/v1/share/get'
    const postData = { share_id }
    const resp = await AliHttp.Post(url, postData, user_id, '')
    if (AliHttp.IsSuccess(resp.code)) {
      return resp.body as IAliShareItem
    } else if (!AliHttp.HttpCodeBreak(resp.code)) {
      DebugLog.mSaveWarning('ApiTransferShareFileStatus err=' + (resp.code || ''), resp.body)
    }
    return false
  }

  static async ApiTransferShareListUntilShareID(user_id: string, share_id: string, limit: number): Promise<boolean> {
    const url = 'adrive/v1/share/list'
    const postData = {
      limit: limit || 20,
      order_by: 'created_at',
      order_direction: 'DESC'
    }
    const resp = await AliHttp.Post(url, postData, user_id, '')
    try {
      if (AliHttp.IsSuccess(resp.code)) {
        for (let i = 0, maxi = resp.body.items.length; i < maxi; i++) {
          const item = resp.body.items[i] as IAliShareItem
          if (item.share_id == share_id) return true
        }
      }
    } catch {}
    return false
  }
}
