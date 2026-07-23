<script lang="ts">
import { computed, h, onMounted, ref } from 'vue'
import { ConfigProvider } from '@arco-design/web-vue'
import enUS from '@arco-design/web-vue/es/locale/lang/en-us'
import zhCN from '@arco-design/web-vue/es/locale/lang/zh-cn'
import { useLocale } from './i18n'
import { useAppStore } from './store'
import PageLoading from './layout/PageLoading.vue'
import PageMain from './layout/PageMain.vue'
import './assets/global.css'
import './assets/fileitem.css'
import './assets/antd.css'
import PageVideoXBTVue from './layout/PageVideoXBT.vue'
import PageCode from './layout/PageCode.vue'
import PageOffice from './layout/PageOffice.vue'
import PagePdf from './layout/PagePdf.vue'
import PageEpub from './layout/PageEpub.vue'
import PageBookReader from './layout/PageBookReader.vue'
import PageDocx from './layout/PageDocx.vue'
import PageSheet from './layout/PageSheet.vue'
import PageImage from './layout/PageImage.vue'
import PageVideo from './layout/PageVideo.vue'
import PageMusic from './layout/PageMusic.vue'
import PageLyric from './lyric/PageLyric.vue'
import PageWorker from './layout/PageWorker.vue'

function shouldShowPageLoadingSplash() {
  if (typeof window === 'undefined') return false
  const splash = new URLSearchParams(window.location.search).get('splash')
  return splash === 'app' || splash === 'music'
}

const PAGE_LOADING_SPLASH_MIN_MS = 1200

export default {
  setup() {
    const appStore = useAppStore()
    const locale = useLocale()
    const arcoLocale = computed(() => locale.value === 'en-US' ? enUS : zhCN)
    const showStartupSplash = shouldShowPageLoadingSplash()
    const splashReady = ref(!showStartupSplash)

    onMounted(() => {
      if (!showStartupSplash) return
      window.setTimeout(() => {
        splashReady.value = true
      }, PAGE_LOADING_SPLASH_MIN_MS)
    })

    return () => {
      let page
      if (!splashReady.value) page = h(PageLoading)
      else if (appStore.appPage == 'PageMain') page = h(PageMain)
      else if (appStore.appPage == 'PageOffice') page = h(PageOffice)
      else if (appStore.appPage == 'PagePdf') page = h(PagePdf)
      else if (appStore.appPage == 'PageEpub') page = h(PageEpub)
      else if (appStore.appPage == 'PageBookReader') page = h(PageBookReader)
      else if (appStore.appPage == 'PageDocx') page = h(PageDocx)
      else if (appStore.appPage == 'PageSheet') page = h(PageSheet)
      else if (appStore.appPage == 'PageVideoXBT') page = h(PageVideoXBTVue)
      else if (appStore.appPage == 'PageCode') page = h(PageCode)
      else if (appStore.appPage == 'PageImage') page = h(PageImage)
      else if (appStore.appPage == 'PageVideo') page = h(PageVideo)
      else if (appStore.appPage == 'PageMusic') page = h(PageMusic)
      else if (appStore.appPage == 'PageLyric') page = h(PageLyric)
      else if (appStore.appPage == 'PageWorker') page = h(PageWorker)
      else if (shouldShowPageLoadingSplash()) page = h(PageLoading)
      else page = h('div', { class: 'desktop-loading-empty' })
      return h(ConfigProvider, { locale: arcoLocale.value }, () => page)
    }
  }
}
</script>

<style>
.desktop-loading-empty {
  position: absolute;
  inset: 0;
  background: transparent;
}
</style>
