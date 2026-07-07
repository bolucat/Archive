<script setup lang="ts">
import { ref } from 'vue'
import useSettingStore from './settingstore'
import { AriaChangeToLocal, AriaChangeToRemote, AriaTest, AriaApplyAdvancedOptions } from '../utils/aria2c'
import { fetchTrackerSource, normalizeTrackerText } from '../down/integration/tracker'
import message from '../utils/message'

import { Checkbox as AntdCheckbox } from 'ant-design-vue'

const settingStore = useSettingStore()
const cb = (val: any) => {
  settingStore.updateStore(val)
}
const ariaState = ref(settingStore.ariaState)
const ariaLoading = ref(settingStore.ariaLoading)
const ariaSavePath = ref(settingStore.ariaSavePath)
const ariaUrl = ref(settingStore.ariaUrl)
const ariaPwd = ref(settingStore.ariaPwd)
const trackerSyncing = ref(false)

const handleAriaConn = () => {
  ariaSavePath.value = ariaSavePath.value.trim()
  if (!ariaSavePath.value || (ariaSavePath.value.indexOf('/') < 0 && ariaSavePath.value.indexOf('\\') < 0)) {
    message.error('下载保存 必须包含路径分隔符 \\或者/')
    return
  }

  let val2 = ariaUrl.value
  val2 = val2.replaceAll('：', ':')
  if (val2.indexOf('://') > 0) val2 = val2.substring(val2.indexOf('://') + 3)
  if (val2.indexOf('/js') > 0) val2 = val2.substring(0, val2.indexOf('/js'))
  ariaUrl.value = val2.trim()
  if (!ariaUrl || ariaUrl.value.indexOf(':') < 0) {
    message.error('连接地址 必须包含 :')
    return
  }

  ariaPwd.value = ariaPwd.value.trim()
  if (!ariaPwd) {
    message.error('连接密码 不能为空，不能包含特殊字符')
    return
  }

  settingStore.updateStore({ ariaSavePath: ariaSavePath.value, ariaUrl: ariaUrl.value, ariaPwd: ariaPwd.value, ariaLoading: true })



  try {
    const host = ariaUrl.value.split(':')[0]
    const port = parseInt(ariaUrl.value.split(':')[1])
    const secret = ariaPwd.value

    AriaTest(settingStore.ariaHttps, host, port, secret).then((issuccess: boolean) => {
      if (issuccess) {
        settingStore.updateStore({ ariaState: 'remote' })
        AriaChangeToRemote().then((isOnline: boolean | undefined) => {
          settingStore.ariaLoading = false
          if (isOnline == true) {
            message.success('连接到远程Aria服务器：成功！')
          } else if (isOnline == undefined) {
            message.warning('连接到远程Aria服务器：正忙，稍后再试！')
          } else {
            message.error('连接到远程Aria服务器：失败！')
          }
        })
      } else {
        settingStore.ariaLoading = false
      }
    })
  } catch (e: any) {
    settingStore.ariaLoading = false
    message.error('数据格式错误！' + e.message)
  }
}
const handleAriaOff = (tip: boolean) => {
  settingStore.updateStore({ ariaState: 'local', ariaLoading: true })

  AriaChangeToLocal()
    .then((isOnline: boolean) => {
      settingStore.ariaLoading = false
      if (tip) {
        if (isOnline) message.warning('已经从远程断开，并连接到本地Aria')
        else message.error('已经从远程断开，连接到本地Aria失败')
      }
    })
    .catch(() => {
      settingStore.ariaLoading = false
      message.error('已经从远程断开，连接到本地Aria失败')
    })
}

const handleSyncTrackers = async () => {
  trackerSyncing.value = true
  try {
    const texts = await Promise.all(
      settingStore.ariaTrackerSources.map((url) => fetchTrackerSource(url))
    )
    const ariaBtTracker = normalizeTrackerText(texts.join('\n'))
    settingStore.updateStore({ ariaBtTracker })
    await AriaApplyAdvancedOptions()
    message.success('Tracker 已同步')
  } catch (error: any) {
    message.error(error?.message || 'Tracker 同步失败')
  } finally {
    trackerSyncing.value = false
  }
}
</script>

<template>
  <div class="settingcard">
    <a-alert banner>可以 把文件直接下载到远程电脑里</a-alert>
    <div class="settingspace"></div>

    <div class="settinghead">Aria远程下载文件保存位置</div>
    <div class="settingrow">
      <a-input tabindex="-1" :disabled="!settingStore.AriaIsLocal" :style="{ width: '300px' }" placeholder="粘贴远程电脑上的文件夹路径" v-model:model-value="ariaSavePath" />
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            这里是远程电脑上的，下载文件的保存路径，需要你手动填写
            <br />
            例如：<span class="opblue">/home/user/Desktop</span>就是把文件下载到远程电脑的桌面
            <div class="hrspace"></div>
            <span class="oporg">注意：</span> windows路径分隔符为<span class="opblue">\</span>，macOS&Linux 为<span class="opblue">/</span> 千万别填错
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">Aria连接地址RPC IP:Port 或 域名:Port</div>
    <div class="settingrow">
      <a-input tabindex="-1" :disabled="!settingStore.AriaIsLocal" :style="{ width: '300px' }" placeholder="Aria2连接地址（IP:Port）" v-model:model-value="ariaUrl">
        <template #prefix> ws:// </template>
        <template #suffix> /jsonrpc </template>
      </a-input>

      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            例如：<span class="opblue">43.211.17.85:6800</span> 手动输入，只填IP和端口号，IP:Port <br />
            例如：<span class="opblue">aria2.yuming.com:6800</span> 手动输入，只填域名和端口号，IP:Port
            <br />
            小白羊会使用ws://IP:Port/jsonrpc 和 http://IP:Port/jsonrpc 连接到Aria
            <div class="hrspace"></div>
            <span class="oporg">注意：</span> 默认没勾选使用ssl，如果你的Aria2开启了https会连接失败
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">Aria连接密码 secret</div>
    <div class="settingrow">
      <a-input tabindex="-1" :disabled="!settingStore.AriaIsLocal" :style="{ width: '300px' }" placeholder="Aria2连接密码" v-model:model-value="ariaPwd" />
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            例如：<span class="opblue">S4znWTaZYQi3cpRN</span> <br />
            必填，不支持空密码<br />
            密码不要包含特殊字符，会连接失败
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">Aria使用ssl链接</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaHttps" @change="(e:any)=>cb({ ariaHttps: e.target.checked })">使用ssl链接(wss 或 https)</AntdCheckbox>

      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">不勾选</span><br />
            勾选后，会使用ssl链接( wss 或 https )<br />
            <span class="oporg">注意：</span> 需要自己在远程aria电脑上配置好证书 <br />
            仅支持域名证书，不支持自己发布的私有证书
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">Aria连接状态</div>
    <div class="settingrow" v-show="settingStore.AriaIsLocal">
      <a-button type="outline" size="small" tabindex="-1" :loading="settingStore.ariaLoading" @click="handleAriaConn">当前是 本地模式，点击切换</a-button>
    </div>
    <div class="settingrow" v-show="!settingStore.ariaLoading && settingStore.AriaIsLocal">
      <a-typography-text type="secondary">新创建的下载任务都会下载到本地，只能管理本地的下载任务</a-typography-text>
    </div>

    <div class="settingrow" v-show="!settingStore.AriaIsLocal">
      <a-button type="primary" size="small" tabindex="-1" :loading="settingStore.ariaLoading" @click="handleAriaOff(false)">当前是 远程Aria模式，点击切换</a-button>
    </div>
    <div class="settingrow" v-show="!settingStore.ariaLoading && !settingStore.AriaIsLocal">
      <a-typography-text type="secondary">远程模式，新创建的下载任务都会下载到Aria服务器上，不会下载到本地，只能管理远程的下载任务。注意：远程下载时不能退出小白羊</a-typography-text>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">BT Tracker</div>
    <div class="settingrow">
      <a-textarea
        :model-value="settingStore.ariaBtTracker"
        :auto-size="{ minRows: 3, maxRows: 6 }"
        placeholder="每行一个 tracker URL"
        @update:model-value="(v: string) => cb({ ariaBtTracker: v })"
        style="width: 460px; font-size: 12px"
      />
    </div>
    <div class="settingrow">
      <a-button :loading="trackerSyncing" size="small" type="outline" tabindex="-1" @click="handleSyncTrackers">立即同步 Tracker</a-button>
      <span class="settingitem">（每 12 小时自动同步一次）</span>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaAutoSyncTracker" @change="(e:any)=>cb({ ariaAutoSyncTracker: e.target.checked })">启动时自动同步 BT Tracker</AntdCheckbox>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">上传限速（KB/s，0 = 不限）</div>
    <div class="settingrow">
      <a-input-number
        tabindex="-1"
        :model-value="settingStore.ariaMaxOverallUploadLimit"
        :min="0"
        :step="100"
        :style="{ width: '140px' }"
        @update:model-value="(v: number) => cb({ ariaMaxOverallUploadLimit: v || 0 })"
      />
      <span class="settingitem">KB/s</span>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">BT 做种设置</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaKeepSeeding" @change="(e:any)=>cb({ ariaKeepSeeding: e.target.checked })">一直做种（忽略比例和时间限制）</AntdCheckbox>
    </div>
    <div class="settingrow" v-show="!settingStore.ariaKeepSeeding">
      <span class="settinglabel">做种比例</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaSeedRatio" :min="0" :step="0.5" :style="{ width: '100px' }" @update:model-value="(v: number) => cb({ ariaSeedRatio: v || 0 })" />
      <span class="settingitem">倍</span>
      <span class="settinglabel" style="margin-left: 16px">做种时间</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaSeedTime" :min="0" :step="60" :style="{ width: '100px' }" @update:model-value="(v: number) => cb({ ariaSeedTime: v || 0 })" />
      <span class="settingitem">分钟</span>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaResumeAllWhenLaunched" @change="(e:any)=>cb({ ariaResumeAllWhenLaunched: e.target.checked })">启动时自动恢复未完成任务</AntdCheckbox>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">浏览器扩展对接</div>
    <div class="settingrow">
      <span class="settinglabel">RPC 地址</span>
      <a-input readonly :model-value="`http://localhost:${settingStore.ariaRpcListenPort}/jsonrpc`" :style="{ width: '260px' }" />
    </div>
    <div class="settingrow">
      <span class="settinglabel">密钥（Token）</span>
      <a-input readonly :model-value="settingStore.ariaRpcSecret" :style="{ width: '260px' }" />
    </div>
    <div class="settingrow">
      <a-alert type="info" content="将以上地址和密钥填入 Aria2 for Chrome 等扩展的设置中，即可从浏览器直接发送下载任务" />
    </div>
  </div>
</template>

<style></style>
