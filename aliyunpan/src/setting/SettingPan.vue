<script setup lang="ts">
import { onMounted, ref, computed } from 'vue'
import useSettingStore from './settingstore'
import MySwitch from '../layout/MySwitch.vue'
import UserDAL from '../user/userdal'
import type { ITokenInfo } from '../user/userstore'

const settingStore = useSettingStore()
const cb = (val: any) => {
  settingStore.updateStore(val)
}

const userList = ref<ITokenInfo[]>([])

const refreshUserList = async () => {
  userList.value = await UserDAL.GetUserListFromDB()
}

onMounted(() => {
  refreshUserList().catch(() => {})
})

const tokenLabel = (t: ITokenInfo) => {
  const provider =
    t.tokenfrom === 'aliyun' ? '阿里云盘' :
    t.tokenfrom === 'cloud123' ? '123 网盘' :
    t.tokenfrom === '115' ? '115 网盘' :
    t.tokenfrom === 'baidu' ? '百度网盘' :
    t.tokenfrom === 'pikpak' ? 'PikPak' :
    t.tokenfrom === 'dropbox' ? 'Dropbox' :
    t.tokenfrom === 'onedrive' ? 'OneDrive' :
    t.tokenfrom === 'box' ? 'Box' :
    '云盘'
  const name = t.nick_name || t.user_name || t.user_id
  return `${provider} · ${name}`
}

const isMusicOn = (uid: string) => !(settingStore.uiLibraryAutoScanMusicDisabledUsers || []).includes(uid)
const isVideoOn = (uid: string) => !(settingStore.uiLibraryAutoScanVideoDisabledUsers || []).includes(uid)

const toggleMusicForUser = (uid: string, on: boolean) => {
  const list = new Set(settingStore.uiLibraryAutoScanMusicDisabledUsers || [])
  if (on) list.delete(uid)
  else list.add(uid)
  cb({ uiLibraryAutoScanMusicDisabledUsers: Array.from(list) })
}

const toggleVideoForUser = (uid: string, on: boolean) => {
  const list = new Set(settingStore.uiLibraryAutoScanVideoDisabledUsers || [])
  if (on) list.delete(uid)
  else list.add(uid)
  cb({ uiLibraryAutoScanVideoDisabledUsers: Array.from(list) })
}

const allMusicOff = () => {
  const ids = userList.value.map((t) => t.user_id).filter(Boolean)
  cb({ uiLibraryAutoScanMusicDisabledUsers: ids })
}
const allMusicOn = () => {
  cb({ uiLibraryAutoScanMusicDisabledUsers: [] })
}
const allVideoOff = () => {
  const ids = userList.value.map((t) => t.user_id).filter(Boolean)
  cb({ uiLibraryAutoScanVideoDisabledUsers: ids })
}
const allVideoOn = () => {
  cb({ uiLibraryAutoScanVideoDisabledUsers: [] })
}

const removeMusicFolder = (f: { user_id: string; drive_id: string; file_id: string }) => {
  const list = (settingStore.uiMusicAutoScanFolders || []).filter(
    (x: any) => !(x.user_id === f.user_id && x.drive_id === f.drive_id && x.file_id === f.file_id)
  )
  cb({ uiMusicAutoScanFolders: list })
}

const hasUsers = computed(() => userList.value.length > 0)
const showAccountList = computed(() =>
  (settingStore.uiLibraryAutoScanMusic || settingStore.uiLibraryAutoScanVideo) && hasUsers.value
)
</script>

<template>
  <div class="settingcard">
    <div class="settings-panel-intro">
      <div class="settings-panel-kicker">Cloud Drive</div>
      <div class="settings-panel-copy">调整文件排序、路径展示、分享模板和标签体系，让网盘主页更贴合你的使用习惯。</div>
    </div>
    <div class="settinghead">优先显示文件夹</div>
    <div class="settingrow">
      <a-select tabindex="-1" :style="{ width: '252px' }" :model-value="settingStore.uiShowPanRootFirst"
                :popup-container="'#SettingDiv'" @update:model-value="cb({ uiShowPanRootFirst: $event })">
        <a-option value="all">所有</a-option>
        <a-option value="backup">备份盘</a-option>
        <a-option value="resource">资源盘</a-option>
      </a-select>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">顶部显示网盘路径</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiShowPanPath" @update:value="cb({ uiShowPanPath: $event })">在顶部显示完整的文件夹路径</MySwitch>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">文件列表显示附属信息</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiShowPanMedia" @update:value="cb({ uiShowPanMedia: $event })">在右侧文件列表中显示每个文件的（播放时长、分辨率）</MySwitch>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">文件夹悬浮预览</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiFolderPreviewEnabled" @update:value="cb({ uiFolderPreviewEnabled: $event })">鼠标悬停文件夹时，弹出窗口预览文件夹内的文件</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">开启</span>
            <hr />
            开启后，鼠标停留在文件夹上 0.45 秒会弹出缩略图预览面板<br />
            关闭后将完全禁用该悬浮预览效果（左侧目录树和右侧文件列表均生效）
          </div>
        </template>
      </a-popover>
    </div>
    <div v-if="settingStore.uiFolderPreviewEnabled" class="settingrow">
      <span style="margin-right: 12px; color: var(--color-text-2)">自动消失时间</span>
      <a-select tabindex="-1" :style="{ width: '160px' }"
                :model-value="settingStore.uiFolderPreviewAutoHide"
                :popup-container="'#SettingDiv'"
                @update:model-value="cb({ uiFolderPreviewAutoHide: $event })">
        <a-option :value="0">不自动消失</a-option>
        <a-option :value="3">3 秒</a-option>
        <a-option :value="6">6 秒（推荐）</a-option>
        <a-option :value="10">10 秒</a-option>
        <a-option :value="20">20 秒</a-option>
      </a-select>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            预览面板出现后，过这段时间会自动消失<br />
            将鼠标移入面板时计时暂停，移出后重新计时
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">自动统计文件夹体积</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiFolderSize" @update:value="cb({ uiFolderSize: $event })">自动统计并显示文件夹的总体积</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">开启</span>
            <hr />
            开启后，小白羊会在后台计算文件夹的体积<br />
            在文件列表里会显示文件夹的总体积 (子文件 + 子文件夹)
            <div class="hrspace"></div>
            <span class="oporg">注：</span>文件夹体积不是时时更新的，会有误差，定时自动更新
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">媒体库后台自动扫描</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiLibraryAutoScanMusic" @update:value="cb({ uiLibraryAutoScanMusic: $event })">音乐库：启动后后台刮削网盘内的音频文件</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">关闭</span>
            <hr />
            开启后，每次打开 App 会按设定间隔在后台静默扫描音频文件并入库<br />
            扫描进行时底部状态栏会显示静默进度条，可点击进入音乐库<br />
            首次登录新网盘账号时会单独弹窗征求同意
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiLibraryAutoScanVideo" @update:value="cb({ uiLibraryAutoScanVideo: $event })">视频媒体库：启动后后台刮削网盘内的视频文件</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">关闭</span>
            <hr />
            开启后，每次打开 App 会按设定间隔在后台对"媒体库 → 文件源"中已添加的所有文件夹进行重扫<br />
            适合定期把新增视频自动收录进媒体库，避免遗漏
          </div>
        </template>
      </a-popover>
    </div>
    <div v-if="settingStore.uiLibraryAutoScanMusic || settingStore.uiLibraryAutoScanVideo" class="settingrow">
      <MySwitch :value="settingStore.uiLibraryIncrementalScan" @update:value="cb({ uiLibraryIncrementalScan: $event })">仅扫描增量（建议开启，按时间间隔节流，避免每次启动重跑）</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">开启</span>
            <hr />
            开启：当距离上次扫描的时间小于"扫描间隔"，本次自动跳过<br />
            关闭：每次打开 App 都立刻发起一次完整扫描（不推荐，开销较大）
          </div>
        </template>
      </a-popover>
    </div>
    <div v-if="(settingStore.uiLibraryAutoScanMusic || settingStore.uiLibraryAutoScanVideo) && settingStore.uiLibraryIncrementalScan" class="settingrow">
      <span style="margin-right: 12px; color: var(--color-text-2)">扫描间隔</span>
      <a-select tabindex="-1" :style="{ width: '180px' }"
                :model-value="settingStore.uiLibraryScanIntervalHours"
                :popup-container="'#SettingDiv'"
                @update:model-value="cb({ uiLibraryScanIntervalHours: $event })">
        <a-option :value="1">1 小时</a-option>
        <a-option :value="6">6 小时</a-option>
        <a-option :value="12">12 小时</a-option>
        <a-option :value="24">24 小时（推荐）</a-option>
        <a-option :value="72">3 天</a-option>
        <a-option :value="168">7 天</a-option>
      </a-select>
    </div>
    <div class="settingrow">
      <MySwitch :value="settingStore.uiLibraryFollowManualScans" @update:value="cb({ uiLibraryFollowManualScans: $event })">媒体源文件夹自动更新（推荐）</MySwitch>
      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">开启</span>
            <hr />
            开启后，你右键"扫描视频/扫描音频"过的文件夹会被记住<br />
            下次打开 App 时即使总开关未开，也会对这些文件夹做增量扫描<br />
            关闭后这些文件夹不会自动重扫
          </div>
        </template>
      </a-popover>
    </div>
    <div v-if="settingStore.uiLibraryFollowManualScans && (settingStore.uiMusicAutoScanFolders || []).length" class="settingrow library-scan-account-row">
      <div class="library-scan-account-head">
        <span style="color: var(--color-text-2); font-weight: 600">音频手动扫描文件夹（共 {{ settingStore.uiMusicAutoScanFolders.length }}）</span>
        <a-popconfirm content="清空所有音频自动扫描文件夹？" @ok="cb({ uiMusicAutoScanFolders: [] })">
          <a-button type="text" size="mini" status="warning">全部清空</a-button>
        </a-popconfirm>
      </div>
      <div class="library-scan-account-list">
        <div v-for="f in settingStore.uiMusicAutoScanFolders" :key="`${f.user_id}|${f.drive_id}|${f.file_id}`" class="library-scan-account-item">
          <div class="library-scan-account-name" :title="f.path || f.name">
            <span style="color: var(--color-text-3); margin-right: 6px">{{ f.path || '/' }}</span>
            {{ f.name || f.file_id }}
          </div>
          <a-button type="text" size="mini" status="danger" @click="removeMusicFolder(f)">移除</a-button>
        </div>
      </div>
    </div>
    <div v-if="showAccountList" class="settingrow library-scan-account-row">
      <div class="library-scan-account-head">
        <span style="color: var(--color-text-2); font-weight: 600">参与扫描的账号</span>
        <div class="library-scan-account-actions">
          <a-button v-if="settingStore.uiLibraryAutoScanMusic" type="text" size="mini" @click="allMusicOn">全开音乐</a-button>
          <a-button v-if="settingStore.uiLibraryAutoScanMusic" type="text" size="mini" status="warning" @click="allMusicOff">全关音乐</a-button>
          <a-button v-if="settingStore.uiLibraryAutoScanVideo" type="text" size="mini" @click="allVideoOn">全开视频</a-button>
          <a-button v-if="settingStore.uiLibraryAutoScanVideo" type="text" size="mini" status="warning" @click="allVideoOff">全关视频</a-button>
        </div>
      </div>
      <div class="library-scan-account-list">
        <div v-for="t in userList" :key="t.user_id" class="library-scan-account-item">
          <div class="library-scan-account-name" :title="tokenLabel(t)">{{ tokenLabel(t) }}</div>
          <div class="library-scan-account-toggles">
            <span v-if="settingStore.uiLibraryAutoScanMusic" class="library-scan-toggle">
              <span class="library-scan-toggle-label">音乐</span>
              <MySwitch :value="isMusicOn(t.user_id)" @update:value="toggleMusicForUser(t.user_id, $event)">&nbsp;</MySwitch>
            </span>
            <span v-if="settingStore.uiLibraryAutoScanVideo" class="library-scan-toggle">
              <span class="library-scan-toggle-label">视频</span>
              <MySwitch :value="isVideoOn(t.user_id)" @update:value="toggleVideoForUser(t.user_id, $event)">&nbsp;</MySwitch>
            </span>
          </div>
        </div>
      </div>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">每个文件夹独立排序</div>
    <div class="settingrow">
      <a-select tabindex="-1" :style="{ width: '252px' }" :model-value="settingStore.uiFileOrderDuli" :popup-container="'#SettingDiv'" @update:model-value="cb({ uiFileOrderDuli: $event })">
        <a-option value="null">
          不开启文件夹的独立排序
          <template #suffix>推荐</template>
        </a-option>
        <a-option value="name asc">开启&默认文件名 升序</a-option>
        <a-option value="name desc">开启&默认文件名 降序</a-option>
        <a-option value="updated_at asc">开启&默认时间 升序</a-option>
        <a-option value="updated_at desc">开启&默认时间 降序</a-option>
        <a-option value="size asc">开启&默认大小 升序</a-option>
        <a-option value="size desc">开启&默认大小 降序</a-option>
      </a-select>
    </div>
  </div>
  <div class="settingcard">
    <div class="settinghead">新建日期文件夹模板</div>
    <div class="settingrow">
      <a-input tabindex="-1" :style="{ width: '257px' }" placeholder="yyyy-MM-dd HH-mm-ss" allow-clear :model-value="settingStore.uiTimeFolderFormate" @update:model-value="cb({ uiTimeFolderFormate: $event })" />
      <a-input-number tabindex="-1" :style="{ width: '100px', marginLeft: '16px', marginTop: '-1px' }" :min="1" :model-value="settingStore.uiTimeFolderIndex" @update:model-value="cb({ uiTimeFolderIndex: $event })" />

      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div style="min-width: 400px">
            默认：<span class="opred">默认yyyy-MM-dd HH-mm-ss</span>(2021-08-08 12-30-00)
            <hr />
            这里是编写命名模板，创建文件夹时会自动替换成当前时间对应的内容
            <br />
            年=<span class="oporg">yyyy</span> 月=<span class="oporg">MM</span> 日=<span class="oporg">dd</span> 时=<span class="oporg">HH</span> 分= <span class="oporg">mm</span> 秒= <span class="oporg">ss</span> 编号=<span class="oporg">#</span>
            <div class="hrspace"></div>
            在这里可以修改编号起始数字，每次成功创建文件夹编号会自动+1
            <br />
            编号可以通过多个#来设置最短的长度
            <div class="hrspace"></div>
            例如:<span class="oporg">#### 创建于yyyy年MM月dd日</span> --&gt;
            <span class="opblue">0001 创建于2021年08月08日</span>
            <br />
            例如:<span class="oporg">yyyy年MM月相册 ##</span> --&gt;
            <span class="opblue">2021年08月相册 01</span>
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">新建分享链接 有效期/提取码</div>
    <div class="settingrow flex">
      <a-radio-group type="button" tabindex="-1" :model-value="settingStore.uiShareDays" @update:model-value="cb({ uiShareDays: $event })">
        <a-radio tabindex="-1" value="always">永久</a-radio>
        <a-radio tabindex="-1" value="week">一周</a-radio>
        <a-radio tabindex="-1" value="month">一月</a-radio>
      </a-radio-group>

      <div style="margin-right: 8px"></div>

      <a-radio-group type="button" tabindex="-1" :model-value="settingStore.uiSharePassword" @update:model-value="cb({ uiSharePassword: $event })">
        <a-radio tabindex="-1" value="random">随机</a-radio>
        <a-radio tabindex="-1" value="last">上次</a-radio>
        <a-radio tabindex="-1" value="nopassword">无提取码</a-radio>
      </a-radio-group>

      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            默认：<span class="opred">永久</span>，<span class="opred">随机</span>
            <hr />
            <span class="opred">永久</span>：新建分享链接永久有效
            <br />
            <span class="opred">一周</span>：新建分享链接7天内有效
            <br />
            <span class="opred">一月</span>：新建分享链接30天内有效
            <br />
            <div class="hrspace"></div>
            <span class="opred">随机</span>：随机生成4位数字字母组合
            <br />
            <span class="opred">上次</span>：上一次创建分享链接时填写的密码
            <br />
            <span class="opred">无提取码</span>：没有提取码
            <br />
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">复制分享链接模板</div>
    <div class="settingrow">
      <a-input tabindex="-1" :style="{ width: '257px' }" placeholder="「NAME」URL 提取码：PWD" allow-clear :model-value="settingStore.uiShareFormate" @update:model-value="cb({ uiShareFormate: $event })" />

      <a-popover position="bottom">
        <IconFont name="iconbulb" />
        <template #content>
          <div style="min-width: 400px">
            默认：<span class="opred">「NAME」URL 提取码：PWD</span> <br />
            测试分享 链接：https://www.aliyundrive.com/s/jEmmmDkF 提取码：DNJI
            <hr />
            这里是编写链接模板，网盘内点击复制分享链接时会自动替换成对应的内容
            <br />
            <span class="oporg">NAME</span>=分享链接标题 <span class="oporg">URL</span>=链接Url <span class="oporg">PWD</span>提取码 <span class="oporg">\n</span>=换行

            <div class="hrspace"></div>
            例如:<span class="oporg">URL#PWD#NAME</span> --&gt; <br />
            <span class="opblue">https://www.aliyundrive.com/s/jEmmmDkF#DNJI#测试分享</span>
            <br />
            例如:<span class="oporg">URL 提取码：PWD NAME</span> --&gt; <br />
            <span class="opblue">https://www.aliyundrive.com/s/jEmmmDkF 提取码：DNJI 测试分享</span>
          </div>
        </template>
      </a-popover>
    </div>
  </div>
  <div class="settingcard">
    <div class="settinghead">
      文件标记 自定义标签名
      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            给文件打上标签，便于分类和快速访问<br />
            支持多地点自动同步(在家里打标、在公司查看)<br />
            支持在这里修改标签的名称<br />
            <div class="hrspace"></div>
            <span class="oporg">轻量使用：</span>不要花大量时间给大量文件打标签，<br />因为不使用小白羊就看不到这些标签了
            <br />
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingrow">
      <a-row class="grid-demo">
        <a-col v-for="item in settingStore.uiFileColorArray" :key="item.key" flex="210px">
          <span style="width: 82px; display: inline-block"><IconFont name="iconcheckbox-full" :style="{ color: item.key }" />{{ item.key }}</span>
          <a-input :style="{ width: '120px' }" allow-clear :model-value="item.title" @update:model-value="(val:string)=>settingStore.updateFileColor(item.key,val)"> </a-input>
        </a-col>
      </a-row>
    </div>
  </div>
</template>

<style scoped>
.settings-panel-intro {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 14px;
}

.settings-panel-kicker {
  display: inline-flex;
  align-self: flex-start;
  padding: 4px 10px;
  border-radius: 999px;
  background: rgba(88, 130, 255, 0.12);
  color: var(--color-primary-6);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.settings-panel-copy {
  max-width: 620px;
  color: var(--color-text-2);
  font-size: 14px;
  line-height: 1.7;
}

:global(html.dark) .settings-panel-kicker {
  background: rgba(120, 160, 255, 0.2);
  color: #dbe6ff;
}

.library-scan-account-row {
  flex-direction: column;
  align-items: stretch;
  gap: 8px;
}

.library-scan-account-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.library-scan-account-actions {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}

.library-scan-account-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  border: 1px solid var(--color-border-2);
  border-radius: 8px;
  padding: 8px 12px;
  max-height: 240px;
  overflow-y: auto;
}

.library-scan-account-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 4px 0;
}

.library-scan-account-item + .library-scan-account-item {
  border-top: 1px dashed var(--color-border-2);
}

.library-scan-account-name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--color-text-1);
  font-size: 13px;
}

.library-scan-account-toggles {
  display: flex;
  gap: 12px;
  flex-shrink: 0;
}

.library-scan-toggle {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.library-scan-toggle-label {
  font-size: 12px;
  color: var(--color-text-3);
}
</style>
