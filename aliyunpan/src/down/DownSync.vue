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
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconstart" />{{ t('transfer.newTask') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconpause" />{{ t('transfer.pauseTask') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="icondelete" />{{ t('transfer.deleteTask') }}</a-button>
    </div>

    <div v-if="false" class="toppanbtn">
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconpause" />{{ t('transfer.modifySettings') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="icondelete" />{{ t('transfer.syncLog') }}</a-button>
      <a-button type="text" size="small" tabindex="-1" @click="handleClick"><IconFont name="iconstart" />{{ t('transfer.syncNow') }}</a-button>
    </div>
  </div>
  <div style="height: 9px"></div>
  <div class="toppanarea">
    <div class="cell pr"></div>
  </div>
  <div class="toppanlist" @keydown.space.prevent="() => true">{{ t('transfer.fileSyncTodo') }}</div>
</template>

<style></style>
