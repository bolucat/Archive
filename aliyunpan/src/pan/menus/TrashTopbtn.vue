<script setup lang="ts">
import {
  canClearTrash,
  menuTrashSelectFile,
  topRecoverSelectedFile,
  topRestoreSelectedFile,
  topTrashDeleteAll
} from '../topbtns/topbtn'
import { computed } from 'vue'
import usePanTreeStore from '../pantreestore'
import { supportsTrashPermanentDelete, supportsTrashRestore } from '../../aliapi/providerFeatures'

const props = defineProps({
  dirtype: {
    type: String,
    required: true
  },
  isselected: {
    type: Boolean,
    required: true
  }
})

const panTreeStore = usePanTreeStore()
const showClearTrash = computed(() => props.dirtype === 'trash' && !props.isselected && canClearTrash(panTreeStore.user_id, panTreeStore.drive_id))
const showTrashRestore = computed(() => props.dirtype === 'trash' && props.isselected && supportsTrashRestore(panTreeStore.user_id, panTreeStore.drive_id))
const showTrashPermanentDelete = computed(() => props.dirtype === 'trash' && props.isselected && supportsTrashPermanentDelete(panTreeStore.user_id, panTreeStore.drive_id))

</script>

<template>
  <div v-show="showClearTrash" class="toppanbtn">
    <a-button type="text" size="small" tabindex="-1" class="danger" @click="topTrashDeleteAll"><IconFont name="iconqingkong" />清空回收站
    </a-button>
  </div>
  <div v-show="showTrashRestore || showTrashPermanentDelete" class="toppanbtn">
    <a-button v-if="showTrashRestore" type="text" size="small" tabindex="-1" @click="topRestoreSelectedFile"><IconFont name="iconrecover" />还原选中
    </a-button>
    <a-button v-if="showTrashPermanentDelete" type="text" size="small" tabindex="-1" class="danger" @click="() => menuTrashSelectFile(false, true)"><IconFont name="iconrest" />彻底删除
    </a-button>
  </div>

  <div v-show="dirtype == 'recover' && isselected" class="toppanbtn">
    <a-button type="text" size="small" tabindex="-1" @click="topRecoverSelectedFile"><IconFont name="iconrecover" />恢复选中
    </a-button>
  </div>
</template>
<style></style>
