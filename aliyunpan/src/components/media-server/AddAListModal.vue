<template>
  <a-modal
    :visible="visible"
    :title="t('mediaServer.addAList')"
    :ok-text="t('mediaServer.connectAndScan')"
    width="720px"
    :ok-loading="loading"
    @ok="handleSubmit"
    @cancel="emit('update:visible', false)"
  >
    <div class="alist-form">
      <div class="field-row">
        <div class="field-label">{{ t('mediaServer.name') }}</div>
        <a-input v-model="form.name" :placeholder="t('mediaServer.alistNamePlaceholder')" allow-clear />
      </div>
      <div class="field-row">
        <div class="field-label">{{ t('mediaServer.serviceAddress') }}</div>
        <a-input v-model="form.url" :placeholder="t('mediaServer.alistAddressPlaceholder')" allow-clear />
      </div>
      <div class="field-row">
        <div class="field-label">{{ t('mediaServer.username') }}</div>
        <a-input v-model="form.username" allow-clear />
      </div>
      <div class="field-row">
        <div class="field-label">{{ t('mediaServer.password') }}</div>
        <a-input-password v-model="form.password" allow-clear />
      </div>
      <div class="field-row">
        <div class="field-label">{{ t('mediaServer.mediaDirectory') }}</div>
        <a-input v-model="form.rootPath" :placeholder="t('mediaServer.scanRootPlaceholder')" allow-clear />
      </div>
      <div class="hint-text">{{ t('mediaServer.alistHint') }}</div>
    </div>
  </a-modal>
</template>

<script setup lang="ts">
import { reactive, watch } from 'vue'
import message from '../../utils/message'
import { t } from '../../i18n'

type AListForm = {
  name: string
  url: string
  username: string
  password: string
  rootPath: string
}

const props = defineProps<{ visible: boolean; loading?: boolean; modelValue: AListForm }>()
const emit = defineEmits<{
  (event: 'update:visible', value: boolean): void
  (event: 'update:modelValue', value: AListForm): void
  (event: 'submit'): void
}>()

const form = reactive<AListForm>({ name: '', url: '', username: '', password: '', rootPath: '/' })

watch(() => props.visible, (visible) => {
  if (!visible) return
  Object.assign(form, props.modelValue)
}, { immediate: true })

watch(form, () => emit('update:modelValue', { ...form }), { deep: true })

const handleSubmit = () => {
  if (!form.url.trim() || !form.username.trim() || !form.password.trim()) {
    message.error(t('mediaServer.fillAList'))
    return
  }
  emit('submit')
}
</script>

<style scoped>
.alist-form { display: flex; flex-direction: column; gap: 14px; padding: 8px 4px 2px; }
.field-row { display: grid; grid-template-columns: 88px minmax(0, 1fr); align-items: center; gap: 14px; }
.field-label { color: var(--color-text-2); font-size: 14px; font-weight: 600; }
.hint-text { padding-left: 102px; color: var(--color-text-3); font-size: 13px; line-height: 1.6; }
</style>
