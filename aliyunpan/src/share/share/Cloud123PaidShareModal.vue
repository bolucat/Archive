<script setup lang="ts">
import { ref } from 'vue'
import { useUserStore } from '../../store'
import { apiCloud123PaidShareList, apiCloud123PaidShareUpdate, getCloud123ShareUrl, type Cloud123PaidShareListItem } from '../../cloud123/share'
import { modalCloseAll } from '../../utils/modal'
import message from '../../utils/message'

defineProps<{ visible: boolean }>()
const rows = ref<Cloud123PaidShareListItem[]>([])
const loading = ref(false)
const selected = ref<string[]>([])
const trafficSwitch = ref(1)
const trafficLimitSwitch = ref(2)
const trafficLimit = ref(0)
const load = async () => {
  loading.value = true
  const result = await apiCloud123PaidShareList(useUserStore().user_id, 0, 100)
  loading.value = false
  if (result.error) message.error(result.error)
  else rows.value = result.list
}
const updateTraffic = async () => {
  if (!selected.value.length) return message.info('请先选择付费分享')
  const result = await apiCloud123PaidShareUpdate(useUserStore().user_id, selected.value, trafficSwitch.value, trafficLimitSwitch.value, trafficLimit.value)
  if (!result.success) return message.error(result.error)
  message.success('付费分享流量设置已更新')
  await load()
}
</script>

<template>
  <a-modal :visible="visible" title="123云盘付费分享管理" :footer="false" :unmount-on-close="true" @before-open="load" @cancel="modalCloseAll">
    <div style="width: 760px; max-width: 80vw">
      <div class="toppanbtns" style="height: 32px; margin-bottom: 10px"><a-button size="small" :loading="loading" @click="load">刷新</a-button><span style="margin-left: 12px; color: var(--color-text-3)">名称和价格由创建时确定，仅支持修改流量策略</span></div>
      <a-table :data="rows" :loading="loading" :pagination="false" row-key="shareId" :scroll="{ y: 360 }" v-model:selected-keys="selected" :row-selection="{ type: 'checkbox', showCheckedAll: true }">
        <template #columns>
          <a-table-column title="分享名称" data-index="shareName" />
          <a-table-column title="价格" data-index="payAmount" :width="90"><template #cell="{ record }">{{ record.payAmount }} 元</template></a-table-column>
          <a-table-column title="订单" data-index="orderCnt" :width="70" />
          <a-table-column title="状态" :width="80"><template #cell="{ record }">{{ record.expired ? '已过期' : '有效' }}</template></a-table-column>
          <a-table-column title="链接" :width="70"><template #cell="{ record }"><a-link :href="getCloud123ShareUrl(useUserStore().user_id, record.shareKey)" target="_blank">打开</a-link></template></a-table-column>
        </template>
      </a-table>
      <a-divider />
      <a-space wrap><span>流量策略</span><a-select v-model="trafficSwitch" style="width: 130px"><a-option :value="1">不限制</a-option><a-option :value="2">限制下载</a-option><a-option :value="3">限制预览</a-option><a-option :value="4">全部限制</a-option></a-select><a-select v-model="trafficLimitSwitch" style="width: 110px"><a-option :value="1">按流量</a-option><a-option :value="2">按次数</a-option></a-select><a-input-number v-model="trafficLimit" :min="0" :precision="0" style="width: 110px" /><a-button type="primary" @click="updateTraffic">保存流量设置</a-button></a-space>
    </div>
  </a-modal>
</template>
