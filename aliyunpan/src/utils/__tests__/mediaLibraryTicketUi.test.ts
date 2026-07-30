import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const read = (path: string) => readFileSync(resolve(process.cwd(), path), 'utf8')

describe('media library ticket regressions', () => {
  it('offers local metadata correction alongside AI rescraping', () => {
    const source = read('src/components/MediaLibrary.vue')

    expect(source).toContain('const saveManualMetadata = (updated: MediaLibraryItem) =>')
    expect(source).toContain('mediaStore.replaceMediaItemMetadata(updated)')
    expect(source).toContain('<MediaMetadataEditorModal')
  })

  it('shows quick filtering on every media-list navigation page', () => {
    const source = read('src/components/MediaLibrary.vue')

    expect(source).toContain("'continue', 'continue-watching', 'recent', 'recently-added', 'movies', 'tv', 'tv-shows'")
    expect(source).toContain("'documentary', 'animation', 'unmatched', 'unwatched', 'favorites'")
    expect(source).toContain('return quickSearchCategories.has(category) || showDrillDownBackBar.value || showPlaylistBackBar.value')
    expect(source).toContain('const showResultCount = computed(() => showQuickSearch.value)')
  })

  it('keeps media results visible while a folder scan is running', () => {
    const source = read('src/components/MediaLibrary.vue')

    expect(source).toContain('class="library-scan-status"')
    expect(source).not.toContain('v-else-if="mediaStore.isScanning" class="loading-state"')
    expect(source).toContain('const liveItems = mediaStore.isScanning')
    expect(source).toContain('liveItems.forEach(item => merged.set(String(item.id), item))')
  })

  it('keeps video and book content shells fully inset and gives books one clean header shell', () => {
    const main = read('src/layout/PageMain.vue')
    const books = read('src/layout/PageBookLibrary.vue')

    expect(main).toContain('#xbybody .media-library-pane,\n#xbybody .book-main {')
    expect(main).toContain('height: calc(100% - 36px);')
    expect(main).toContain('#xbybody .book-content {')
    expect(main).toContain('border: 0 !important;')
    expect(books).toContain("class='book-header-title-mark'")
    expect(books).toContain("class='book-total-count'")
  })

  it('mounts the music sound-effect control in the player', () => {
    const source = read('src/layout/PageMusic.vue')

    expect(source).toContain("import SoundEffectBtn from '../components/SoundEffectBtn.vue'")
    expect(source).toContain('<SoundEffectBtn />')
  })

  it('keeps 115 root folders selectable and protects list requests from bursts', () => {
    const picker = read('src/pan/topbtns/SelectPanDirModal.vue')
    const list = read('src/cloud115/dirfilelist.ts')

    expect(picker).toContain('isDrive115User(userId)')
    expect(picker).toContain("file_id: driveType.key")
    expect(picker).toContain('isDir: true')
    expect(picker).toContain("const parentCid = key.includes('root') ? 0 : Number(key)")
    expect(list).toContain('const LIST_REQUEST_GAP_MS = 900')
    expect(list).toContain('const enqueueListRequest')
    expect(list).toContain("params.set('show_dir', showDir ? '1' : '0')")
  })

  it('opens cloud books through the provider-aware download proxy', () => {
    const source = read('src/layout/BookReaderModal.vue')

    expect(source).toContain("getRawUrl(book.user_id, book.drive_id, book.file_id")
    expect(source).toContain('return getProxyUrl({')
    expect(source).toContain('proxy_url: rawData.url')
  })

  it('keeps the selected media-source file list compact without an empty header or fixed blank viewport', () => {
    const library = read('src/components/MediaLibrary.vue')
    const fileList = read('src/components/MediaPanRight.vue')

    expect(library).toContain('v-if="!showingDetail && !isHomeView && !props.selectedFolder"')
    expect(library).toContain('v-else-if="props.selectedFolder && folderFileList.length > 0"')
    expect(fileList).not.toContain(":max-height='500'")
    expect(fileList).not.toContain('height: 500,')
    expect(fileList).toContain(':max-height=\'listViewportHeight\'')
    expect(fileList).toContain('height: listViewportHeight,')
  })

  it('aligns media-source row actions and uses one icon size', () => {
    const source = read('src/components/MediaPanRight.vue')

    expect(source).toContain("class='select media-file-action'")
    expect(source).toContain("class='gengduo media-file-action'")
    expect(source).toContain(':deep(.media-file-action.arco-btn)')
    expect(source).toContain('min-height: 28px;')
    expect(source).toContain('.media-file-action .iconfont')
    expect(source).toContain('font-size: 20px;')
  })
})
