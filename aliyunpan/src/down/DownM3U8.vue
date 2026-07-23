<script setup lang="ts">
import { t } from '../i18n'

const handleClick = () => {}
</script>

<template>
  <div style="height: 7px"></div>
  <div class="toppanbtns" style="height: 26px"></div>
  <div style="height: 14px"></div>
  <div class="toppanbtns" style="height: 26px">
    <div class="toppanbtn">
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconstart" />{{ t('transfer.startAll') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconpause" />{{ t('transfer.pauseAll') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconrest" />{{ t('transfer.clearAll') }}</a-button>
    </div>

    <div v-if="false" class="toppanbtn">
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconstart" />{{ t('transfer.start') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconpause" />{{ t('transfer.pause') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="icondelete" />{{ t('transfer.clear') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconyouxian" />{{ t('transfer.prioritize') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="icondian" /></a-button>

      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconstart" />{{ t('transfer.startAll') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconpause" />{{ t('transfer.pauseAll') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconrest" />{{ t('transfer.clearAll') }}</a-button>
    </div>
  </div>
  <div style="height: 9px"></div>
  <div class="toppanarea">
    <div class="cell pr"></div>
  </div>
  <div class="toppanlist" @keydown.space.prevent="() => true">{{ t('transfer.transcodedVideoTodo') }}</div>
</template>

<style></style>
