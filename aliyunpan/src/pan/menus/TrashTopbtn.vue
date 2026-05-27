<script setup lang="ts">
import {
  menuTrashSelectFile,
  topRecoverSelectedFile,
  topRestoreSelectedFile,
  topTrashDeleteAll
} from '../topbtns/topbtn'

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

</script>

<template>
  <div v-show="dirtype == 'trash' && !isselected" class="toppanbtn">
    <a-button type="text" size="small" tabindex="-1" class="danger" @click="topTrashDeleteAll"><IconFont name="iconqingkong" />清空回收站
    </a-button>
  </div>
  <div v-show="dirtype == 'trash' && isselected" class="toppanbtn">
    <a-button type="text" size="small" tabindex="-1" @click="topRestoreSelectedFile"><IconFont name="iconrecover" />还原选中
    </a-button>
    <a-button type="text" size="small" tabindex="-1" class="danger" @click="() => menuTrashSelectFile(false, true)"><IconFont name="iconrest" />彻底删除
    </a-button>
  </div>

  <div v-show="dirtype == 'recover' && isselected" class="toppanbtn">
    <a-button type="text" size="small" tabindex="-1" @click="topRecoverSelectedFile"><IconFont name="iconrecover" />恢复选中
    </a-button>
  </div>
</template>
<style></style>
