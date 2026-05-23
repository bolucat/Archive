<script setup lang='ts'>
import message from '../utils/message'
import UserDAL, { UserTokenMap } from '../user/userdal'
import { ITokenInfo, useSettingStore, useUserStore } from '../store'
import { copyToClipboard, openExternal } from '../utils/electronhelper'
import Db from '../utils/db'
import fs from 'node:fs'
import path from 'path'
import { decodeName, encodeName } from '../module/flow-enc/utils'
import { localPwd } from '../utils/aria2c'

const settingStore = useSettingStore()

const cb = (val: any) => {
  settingStore.updateStore(val)
}

const openWebUrl = (type: string) => {
  switch (type) {
    case 'developer':
      openExternal('https://www.aliyundrive.com/developer')
      break
    case 'pkce':
      openExternal('https://www.yuque.com/aliyundrive/zpfszx/eam8ls1lmawwwksv')
      break
    case 'AList':
      openExternal('https://alist.nn.ci/tool/aliyundrive/request.html')
      break
  }
}

const copyCookies = async () => {
  let cookies = await window.WebGetCookies({ url: 'https://www.aliyundrive.com' }) as []
  if (cookies.length == 0) cookies = await window.WebGetCookies({ url: 'https://www.aliyundrive.com' }) as []
  if (cookies.length > 0) {
    let cookiesText = ''
    cookies.forEach(cookie => {
      cookiesText += cookie['name'] + '=' + cookie['value'] + ';'
    })
    copyToClipboard(cookiesText)
    message.success('当前账号的Cookies已复制到剪切板')
  } else {
    message.error('当前账号的Cookies不存在')
  }
}

const handlerAccountImport = () => {
  window.WebShowOpenDialogSync({
    title: '选择需要导入的账户文件',
    buttonLabel: '导入选中的账户文件',
    filters: [{ name: 'user.db', extensions: ['db'] }],
    properties: ['openFile', 'multiSelections', 'showHiddenFiles', 'noResolveAliases', 'treatPackageAsDirectory', 'dontAddToRecent']
  }, async (files: string[] | undefined) => {
    if (files && files.length > 0) {
      try {
        // 获取内容
        let userList: ITokenInfo[] = []
        let uniqueUserIds = new Set()
        for (let filePath of files) {
          let readData = fs.readFileSync(filePath, 'utf-8')
          let parsedData: any = JSON.parse(<string>decodeName(localPwd, 'aesctr', readData))
          if (Array.isArray(parsedData) && parsedData.every(item => item.hasOwnProperty('access_token'))) {
            let filteredData: ITokenInfo[] = parsedData.filter((item: ITokenInfo) => {
              if (!uniqueUserIds.has(item.user_id)) {
                uniqueUserIds.add(item.user_id)
                return true
              }
              return false
            })
            userList.push(...filteredData)
          }
        }
        if (userList.length > 0) {
          // 设置UserTokenMap
          for (let token of userList) {
            if (token.user_id) {
              UserTokenMap.set(token.user_id, token)
            }
          }
          // 导入到数据库
          Db.saveUserBatch(userList).then(() => {
            window.WinMsgToUpload({ cmd: 'ClearUserToken' })
            window.WinMsgToDownload({ cmd: 'ClearUserToken' })
          }).catch()
          await UserDAL.UserLogin(userList[0])
          message.success('导入用户账户数据成功')
        } else {
          message.error('数据错误，导入用户账户数据失败')
        }
      } catch (err) {
        message.error('数据错误，导入用户账户数据失败')
      }
    }
  })
}

const handlerAccountExport = () => {
  if (window.WebShowOpenDialogSync) {
    window.WebShowOpenDialogSync(
      {
        title: '选择一个文件夹，保存导出的数据',
        buttonLabel: '选择',
        properties: ['openDirectory', 'createDirectory']
      },
      (result: string[] | undefined) => {
        if (result && result[0]) {
          let exportFile = path.join(result[0], 'user.db')
          let userList = JSON.stringify(UserDAL.GetUserList())
          let data = encodeName(localPwd, 'aesctr', userList)
          fs.writeFileSync(exportFile, data)
          message.success('导出所有用户账户数据成功')
        }
      }
    )
  }
}

const handlerExportCliTokens = async () => {
  const result = await UserDAL.SyncCliAccountsToCli()
  if (result?.ok) {
    message.success(`已导出 ${result.exported} 个账号到 ${result.path}`)
  } else {
    message.error(`导出失败：${result?.error || '未知错误'}`)
  }
}
</script>

<template>
  <div class='settingcard'>
    <div class='settinghead'>阿里云盘账号</div>
    <div class='settingrow'>
      <a-button type='outline' size='small' tabindex='-1' @click='copyCookies()'>
        复制当前账号Cookies
      </a-button>
    </div>
    <div class='settingspace'></div>
    <div class='settinghead'>账号导入导出</div>
    <a-popover position="bottom">
      <i class="iconfont iconbulb" />
      <template #content>
        <div>
          可以一键恢复所有账户的数据（加密）<br />
          <hr />
          <div class="hrspace"></div>
          <span class="opred">批量导入导出所有账户的数据</span><br />
        </div>
      </template>
    </a-popover>
    <div class="settingrow">
      <a-button type='outline' style='margin-right: 12px' status="danger" size='small' tabindex='-1'
                @click='handlerAccountExport'>
        导出账号
      </a-button>
      <a-button type='outline' size='small' status="success" tabindex='-1' @click='handlerAccountImport'>
        导入账号
      </a-button>
      <a-button type='outline' style='margin-left: 12px' size='small' tabindex='-1'
                @click='handlerExportCliTokens'>
        导出到 CLI
      </a-button>
    </div>
  </div>
</template>

<style scoped>

</style>
