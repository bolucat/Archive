<script setup lang="ts">
import { ref } from 'vue'
import UserDAL from '../../user/userdal'
import message from '../../utils/message'
import { normalizeMiaochuanPayload } from '../../utils/drive-tools/miaochuan'
import { apiGuangyaImportMiaochuan } from '../../guangya/miaochuan'
import { extractMagnetLinks, importGuangyaMagnets } from '../../utils/drive-tools/magnet'

const rawText = ref('')
const parentId = ref('guangya_root')
const report = ref('')
const loading = ref(false)
const magnetText = ref('')
const magnetParentId = ref('guangya_root')
const magnetLoading = ref(false)
const magnetReport = ref('')

const handleAnalyze = () => {
  if (!rawText.value.trim()) {
    message.warning('请先粘贴秒传 JSON')
    return
  }
  report.value = normalizeMiaochuanPayload(rawText.value).report
}

const getGuangyaUserId = async () => {
  const users = await UserDAL.GetUserListFromDB()
  const user = users.find((u: any) => u?.tokenfrom === 'guangya' && u?.access_token)
  return user?.user_id || ''
}

const handleImport = async () => {
  if (loading.value) return
  if (!rawText.value.trim()) {
    message.warning('请先粘贴秒传 JSON')
    return
  }
  const parsed = normalizeMiaochuanPayload(rawText.value)
  if (!parsed.files.length) {
    report.value = parsed.report
    message.error('没有可导入的秒传文件')
    return
  }
  const userId = await getGuangyaUserId()
  if (!userId) {
    message.error('请先登录光鸭云盘')
    return
  }

  loading.value = true
  try {
    const result = await apiGuangyaImportMiaochuan(userId, parentId.value || 'guangya_root', parsed.files)
    report.value = [
      parsed.report,
      '',
      `导入完成：成功 ${result.success}/${result.total}，失败 ${result.failed}，跳过 ${result.skipped}`,
      result.failures.length ? `失败示例：${result.failures.slice(0, 5).map(item => `${item.path}(${item.reason})`).join('；')}` : ''
    ].filter(Boolean).join('\n')
    if (result.failed) message.warning('部分文件导入失败，请查看报告')
    else message.success('秒传导入完成')
  } catch (error: any) {
    message.error(error?.message || '秒传导入失败')
  } finally {
    loading.value = false
  }
}

const handleAnalyzeMagnets = () => {
  const magnets = extractMagnetLinks(magnetText.value)
  magnetReport.value = magnets.length
    ? `识别到 ${magnets.length} 条磁力链接\n${magnets.slice(0, 10).join('\n')}${magnets.length > 10 ? '\n…' : ''}`
    : '没有识别到 magnet 链接'
}

const handleImportMagnets = async () => {
  if (magnetLoading.value) return
  const magnets = extractMagnetLinks(magnetText.value)
  if (!magnets.length) {
    magnetReport.value = '没有识别到 magnet 链接'
    message.error('没有可提交的磁力链接')
    return
  }
  const userId = await getGuangyaUserId()
  if (!userId) {
    message.error('请先登录光鸭云盘')
    return
  }
  magnetLoading.value = true
  try {
    const result = await importGuangyaMagnets(userId, magnetParentId.value || 'guangya_root', magnetText.value)
    magnetReport.value = result.report
    if (result.failed) message.warning('部分磁力提交失败，请查看报告')
    else message.success('磁力云添加提交完成')
  } catch (error: any) {
    message.error(error?.message || '磁力云添加失败')
  } finally {
    magnetLoading.value = false
  }
}

</script>

<template>
  <div class="fullscroll rightbg">
    <div class="settingcard">
      <div class="settinghead">秒传 JSON 导入</div>
      <div class="settingrow">
        <a-typography-text type="secondary">这个入口仍兼容 guangya-cloud-helper 生成的秒传 JSON。后续应改成 app 内直接从来源网盘生成导入清单。</a-typography-text>
      </div>
      <div class="settingspace"></div>
      <div class="settingrow">
        <a-input v-model="parentId" placeholder="光鸭目标目录 ID，默认根目录 guangya_root" />
      </div>
      <div class="settingspace"></div>
      <div class="settingrow">
        <a-textarea v-model="rawText" :auto-size="{ minRows: 10, maxRows: 18 }" placeholder="粘贴秒传 JSON..." />
      </div>
      <div class="settingspace"></div>
      <div class="settingrow rss-drive-actions">
        <a-button @click="handleAnalyze">解析清单</a-button>
        <a-button type="primary" :loading="loading" @click="handleImport">导入光鸭</a-button>
      </div>
    </div>

    <div v-if="report" class="settingcard">
      <div class="settinghead">执行报告</div>
      <pre class="rss-drive-report">{{ report }}</pre>
    </div>

    <div class="settingcard">
      <div class="settinghead">磁力云批量添加</div>
      <div class="settingrow">
        <a-typography-text type="secondary">粘贴包含 magnet 链接的文本或 JSON，系统会自动识别并提交到光鸭云添加。</a-typography-text>
      </div>
      <div class="settingspace"></div>
      <div class="settingrow">
        <a-input v-model="magnetParentId" placeholder="光鸭目标目录 ID，默认根目录 guangya_root" />
      </div>
      <div class="settingspace"></div>
      <div class="settingrow">
        <a-textarea v-model="magnetText" :auto-size="{ minRows: 8, maxRows: 16 }" placeholder="粘贴 magnet 链接、文本或 JSON..." />
      </div>
      <div class="settingspace"></div>
      <div class="settingrow rss-drive-actions">
        <a-button @click="handleAnalyzeMagnets">识别磁力</a-button>
        <a-button type="primary" :loading="magnetLoading" @click="handleImportMagnets">提交云添加</a-button>
      </div>
      <div v-if="magnetReport" class="settingspace"></div>
      <pre v-if="magnetReport" class="rss-drive-report">{{ magnetReport }}</pre>
    </div>

  </div>
</template>

<style scoped>
.rss-drive-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.rss-drive-report {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--color-text-2);
  font-size: 13px;
  line-height: 1.7;
}

.rss-drive-list {
  max-height: 240px;
  overflow: auto;
  border: 1px solid var(--color-border-2);
  border-radius: 8px;
  background: var(--color-fill-1);
}

.rss-drive-list-item,
.rss-drive-list-more {
  padding: 6px 10px;
  font-size: 12px;
  color: var(--color-text-2);
  border-bottom: 1px solid var(--color-border-2);
}

.rss-drive-list-more {
  color: var(--color-text-4);
  border-bottom: 0;
}
</style>
