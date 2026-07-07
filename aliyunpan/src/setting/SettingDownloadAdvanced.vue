<script setup lang="ts">
import useSettingStore from './settingstore'
import { Checkbox as AntdCheckbox } from 'ant-design-vue'
import { AriaApplyAdvancedOptions } from '../utils/aria2c'
import { fetchTrackerSource, normalizeTrackerText } from '../down/integration/tracker'
import { ref } from 'vue'
import message from '../utils/message'

const settingStore = useSettingStore()
const cb = (val: any) => settingStore.updateStore(val)
const trackerSyncing = ref(false)

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

const applyAriaOptions = () => AriaApplyAdvancedOptions().catch(() => {})
</script>

<template>
  <div class="settingcard">
    <div class="settinghead">BT 做种</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaKeepSeeding" @change="(e:any) => { cb({ ariaKeepSeeding: e.target.checked }); applyAriaOptions() }">
        一直做种（忽略比例和时间限制）
      </AntdCheckbox>
    </div>
    <div class="settingrow" v-show="!settingStore.ariaKeepSeeding">
      <span class="settinglabel">做种比例</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaSeedRatio" :min="0" :step="0.5" :style="{ width: '90px' }" @update:model-value="(v: number) => { cb({ ariaSeedRatio: v || 0 }); applyAriaOptions() }" />
      <span class="settingitem">倍</span>
      <span class="settinglabel" style="margin-left: 16px">做种时间</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaSeedTime" :min="0" :step="60" :style="{ width: '100px' }" @update:model-value="(v: number) => { cb({ ariaSeedTime: v || 0 }); applyAriaOptions() }" />
      <span class="settingitem">分钟</span>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaBtSaveMetadata" @change="(e:any) => cb({ ariaBtSaveMetadata: e.target.checked })">
        保存 BT 种子元数据
      </AntdCheckbox>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaBtForceEncryption" @change="(e:any) => { cb({ ariaBtForceEncryption: e.target.checked }); applyAriaOptions() }">
        BT 强制加密
      </AntdCheckbox>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaBtAutoDownloadContent" @change="(e:any) => cb({ ariaBtAutoDownloadContent: e.target.checked })">
        自动开始 BT 内容下载（种子内文件自动勾选）
      </AntdCheckbox>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">Tracker 同步</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaAutoSyncTracker" @change="(e:any) => cb({ ariaAutoSyncTracker: e.target.checked })">
        启动时自动同步 BT Tracker（每 12 小时）
      </AntdCheckbox>
    </div>
    <div class="settingrow">
      <a-button :loading="trackerSyncing" size="small" type="outline" tabindex="-1" @click="handleSyncTrackers">立即同步 Tracker</a-button>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">网络与端口</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaEnableUpnp" @change="(e:any) => { cb({ ariaEnableUpnp: e.target.checked }); applyAriaOptions() }">
        UPnP/NAT-PMP 端口映射
      </AntdCheckbox>
    </div>
    <div class="settingrow">
      <span class="settinglabel">BT 监听端口</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaListenPort" :min="1024" :max="65535" :step="1" :style="{ width: '120px' }" @update:model-value="(v: number) => { cb({ ariaListenPort: v || 6881 }); applyAriaOptions() }" />
      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">6881</span>
            <hr />
            BT 下载使用的监听端口<br />
            修改后需重启 aria2 生效
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingrow">
      <span class="settinglabel">DHT 监听端口</span>
      <a-input-number tabindex="-1" :model-value="settingStore.ariaDhtListenPort" :min="1024" :max="65535" :step="1" :style="{ width: '120px' }" @update:model-value="(v: number) => { cb({ ariaDhtListenPort: v || 6881 }); applyAriaOptions() }" />
      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">6881</span>
            <hr />
            DHT 网络使用的 UDP 端口<br />
            修改后需重启 aria2 生效
          </div>
        </template>
      </a-popover>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">传输设置</div>
    <div class="settingrow">
      <span class="settinglabel">全局 User-Agent</span>
      <a-textarea tabindex="-1" :model-value="settingStore.ariaUserAgent" :auto-size="{ minRows: 1, maxRows: 3 }" :style="{ width: '460px' }" placeholder="Aria2 全局 User-Agent，用于所有 HTTP 下载" @update:model-value="(v: string) => cb({ ariaUserAgent: v })" />
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaContinueDownload" @change="(e:any) => cb({ ariaContinueDownload: e.target.checked })">
        断点续传（Continue）
      </AntdCheckbox>
      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">开启</span>
            <hr />
            开启后 aria2 会继续下载未完成的文件<br />
            关闭则每次从头开始下载
          </div>
        </template>
      </a-popover>
    </div>

    <div class="settingspace"></div>
    <div class="settinghead">任务恢复</div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaResumeAllWhenLaunched" @change="(e:any) => cb({ ariaResumeAllWhenLaunched: e.target.checked })">
        启动时自动恢复未完成任务
      </AntdCheckbox>
    </div>
    <div class="settingrow">
      <AntdCheckbox tabindex="-1" :checked="settingStore.ariaTaskNotification" @change="(e:any) => cb({ ariaTaskNotification: e.target.checked })">
        下载完成声音通知
      </AntdCheckbox>
    </div>

<!--    <div class="settingspace"></div>-->
<!--    <div class="settinghead">上传限速（KB/s，0 = 不限）</div>-->
<!--    <div class="settingrow">-->
<!--      <a-input-number tabindex="-1" :model-value="settingStore.ariaMaxOverallUploadLimit" :min="0" :step="100" :style="{ width: '140px' }" @update:model-value="(v: number) => { cb({ ariaMaxOverallUploadLimit: v || 0 }); applyAriaOptions() }" />-->
<!--      <span class="settingitem">KB/s</span>-->
<!--    </div>-->
  </div>
</template>

<style></style>
