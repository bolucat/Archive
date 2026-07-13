<script lang="ts">
import { h, onMounted, ref } from 'vue'
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
    const showStartupSplash = shouldShowPageLoadingSplash()
    const splashReady = ref(!showStartupSplash)

    onMounted(() => {
      if (!showStartupSplash) return
      window.setTimeout(() => {
        splashReady.value = true
      }, PAGE_LOADING_SPLASH_MIN_MS)
    })

    return () => {
      if (!splashReady.value) return h(PageLoading)
      if (appStore.appPage == 'PageMain') return h(PageMain)
      if (appStore.appPage == 'PageOffice') return h(PageOffice)
      if (appStore.appPage == 'PagePdf') return h(PagePdf)
      if (appStore.appPage == 'PageEpub') return h(PageEpub)
      if (appStore.appPage == 'PageBookReader') return h(PageBookReader)
      if (appStore.appPage == 'PageDocx') return h(PageDocx)
      if (appStore.appPage == 'PageSheet') return h(PageSheet)
      if (appStore.appPage == 'PageVideoXBT') return h(PageVideoXBTVue)
      if (appStore.appPage == 'PageCode') return h(PageCode)
      if (appStore.appPage == 'PageImage') return h(PageImage)
      if (appStore.appPage == 'PageVideo') return h(PageVideo)
      if (appStore.appPage == 'PageMusic') return h(PageMusic)
      if (appStore.appPage == 'PageLyric') return h(PageLyric)
      if (appStore.appPage == 'PageWorker') return h(PageWorker)
      if (shouldShowPageLoadingSplash()) return h(PageLoading)
      return h('div', { class: 'desktop-loading-empty' })
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
