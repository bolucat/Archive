<script lang="ts">
import { defineComponent, ref, watchEffect } from 'vue'
import { useWinStore, WinState } from '../store'

export default defineComponent({
  props: {
    visible: {
      type: Boolean,
      required: false,
      default: true
    },
  },
  emits: ['splitSize'],
  setup(props) {
    const leftMinWidth = 0
    const rightMinWidth = 220
    const winStore = useWinStore()
    const bodyWidth = ref(Math.max(winStore.width, 900))
    const splitMoveing = ref(false)
    const splitSize = ref(bodyWidth.value < 900 ? '220px' : '240px')
    const splitSizeMax = ref(bodyWidth.value - rightMinWidth)

    winStore.$subscribe((_m: any, state: WinState) => {
      const width = state.width
      if (width > 0 && bodyWidth.value != width) {
        bodyWidth.value = width
        splitSizeMax.value = width - rightMinWidth
        const tempSize = parseInt(splitSize.value, 10)
        if (tempSize < leftMinWidth) {
          splitSize.value = leftMinWidth.toString() + 'px'
        } else if (tempSize > leftMinWidth && tempSize > splitSizeMax.value) {
          splitSize.value = splitSizeMax.value.toString() + 'px'
        }
      }
    })
    watchEffect(() => {
      if(props.visible){
        splitSize.value = bodyWidth.value < 900 ? '220px' : '240px'
      }else {
        splitSize.value = '0px'
      }
    })
    return { splitSize, leftMinWidth, splitSizeMax, splitMoveing,  }
  }
})
</script>

<template>
  <a-split v-model:size="splitSize" class="MySplit" style="height: 100%; width: 100%;"
           :min="leftMinWidth" :max="splitSizeMax" tabindex="-1"
           @move-start="splitMoveing = true" @move-end="splitMoveing = false">
    <template #first>
      <slot name="first">first</slot>
    </template>
    <template #resize-trigger>
      <div class="splitline" :class="splitMoveing ? 'resize' : ''" draggable="false">
        <div class="line" draggable="false"></div>
      </div>
    </template>
    <template #second>
      <slot name="second">second</slot>
    </template>
  </a-split>
</template>
<style>
.MySplit .arco-split-pane {
  overflow: hidden;
}
.splitline {
  position: relative;
  box-sizing: border-box;
  width: 4px;
  height: 100%;
  border: 0;
  user-select: none;
  margin-right: 2px;
}
.splitline::before {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 1px;
  width: 1px;
  background: rgba(31, 35, 41, 0.3);
  content: '';
}
body[arco-theme='dark'] .splitline::before {
  background: rgba(255, 255, 255, 0.28);
}
.splitline:hover {
  background: rgb(var(--primary-6));
  cursor: col-resize;
}
.splitline:hover::before,
.splitline.resize::before {
  background: transparent;
}
.splitline.resize {
  background: rgb(var(--primary-6));
}
.splitline .line {
  position: absolute;
  top: 50%;
  width: 2px;
  height: 60px;
  margin-top: -30px;
  background: rgb(var(--primary-6));
}
</style>
