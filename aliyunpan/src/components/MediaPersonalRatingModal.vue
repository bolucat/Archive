<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { t } from '../i18n'
import { readPersonalRatings, savePersonalRating } from '../utils/mediaPersonalRating'
import { loadTraktStatus, syncTrakt, traktStatus, traktError } from '../services/trakt/client'
import type { TraktIdentity } from '@shared/trakt'
const target = ref<({ id: string; name: string } & Partial<TraktIdentity>) | null>(null)
const busy = ref(false)
const rating = ref(0)
const error = ref('')
function open(event: Event) {
  if (busy.value) return
  const item = (event as CustomEvent).detail
  if (!item?.id || !item?.name) return
  target.value = item; error.value = ''
  if (window.Electron?.ipcRenderer) void loadTraktStatus().catch(e => { error.value = traktError(e) })
  try { rating.value = readPersonalRatings()[item.id] || 0 } catch { rating.value = 0; error.value = t('posterMenu.ratingSaveError') }
}
function close() { if (!busy.value) target.value = null }
async function save() {
  if (!target.value || !rating.value || busy.value) return
  busy.value = true; error.value = ''
  const item = target.value
  try {
    if (window.Electron?.ipcRenderer && (await loadTraktStatus()).connected) await syncTrakt(item as TraktIdentity, 'rating', rating.value)
    savePersonalRating(item.id, rating.value); target.value = null
  } catch (e) { error.value = String(e).includes('TRAKT_') ? traktError(e) : t('posterMenu.ratingSaveError') }
  finally { busy.value = false }
}
onMounted(() => window.addEventListener('boxplayer:personal-rating', open))
onUnmounted(() => window.removeEventListener('boxplayer:personal-rating', open))
</script>
<template><a-modal :visible="!!target" :title="t('posterMenu.personalRating')" :footer="false" :width="420" :mask-closable="!busy" :esc-to-close="!busy" @cancel="close"><div class="personal-rating"><strong>{{ target?.name }}</strong><a-rate v-model="rating" :count="10" :allow-half="false" :disabled="busy" /><small>{{ t(traktStatus.connected ? 'trakt.ratingNote' : 'posterMenu.personalRatingNote') }}</small><p v-if="error" role="alert">{{ error }}</p><div><a-button :disabled="busy" @click="close">{{ t('common.cancel') }}</a-button><a-button type="primary" :loading="busy" :disabled="rating < 1" @click="save">{{ t('posterMenu.rating') }}</a-button></div></div></a-modal></template>
<style scoped>.personal-rating{display:flex;flex-direction:column;gap:20px}.personal-rating small{color:var(--color-text-3)}.personal-rating>div{display:flex;justify-content:flex-end;gap:12px}.personal-rating :deep(.arco-rate){color:#ff8800}</style>
