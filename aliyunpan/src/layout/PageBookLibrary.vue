<script setup lang='ts'>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import type { CSSProperties } from 'vue'
import { Archive, BarChart3, BookMarked, BookOpen, Bookmark, Copy, Database, Edit3, FileText, Folder, Globe2, Grid3X3, Heart, Highlighter, Info, Languages, LibraryBig, Lightbulb, List, MoreVertical, Palette, PanelLeft, PanelLeftClose, PencilLine, Plus, RefreshCw, Search, Settings, Star, StickyNote, Tag, Trash2, X } from 'lucide-vue-next'
import useBookLibraryStore, { parseBookMeta } from '../store/booklibrary'
import type { IBookItem } from '../types/book'
import type { IBookBookmark } from '../types/bookBookmark'
import type { IBookNote } from '../types/bookNote'
import type { BookShelfGroup, BookViewMode, BookManagerSortMode, BookManagerSortOrder, BookManagerView } from '../types/bookShelf'
import BookScanner from '../utils/bookScanner'
import LibraryScanPanel from '../components/LibraryScanPanel.vue'
import { isReaderFormat, isLegacyScanOnlyBookFormat } from '../utils/bookReaderCapabilities'
import { DEFAULT_SHELF_ID, buildDefaultShelfName, buildNoteLink, filterBooksByReadingStatus, filterBooksForManagerView, filterAnnotations, filterBookmarks, getAnnotationTags, getBookManagerTabs, saveGlobalNoteTags, sortBooksForManagerView, sortAnnotations } from '../utils/bookManagerParity'
import type { BookAnnotationSortMode, ReadingStatus } from '../utils/bookManagerParity'
import { exportAllAnnotations } from '../utils/bookAnnotationExport'
import type { AnnotationExportFormat } from '../utils/bookAnnotationExport'
import message from '../utils/message'
import { humanSize } from '../utils/format'
import { copyToClipboard } from '../utils/electronhelper'
import {
  BOOK_MANAGER_SETTING_TABS,
  BOOK_MANAGER_SETTING_TABS_GROUP2,
  BOOK_MANAGER_THEME_COLORS,
  loadBookManagerPreferences,
  saveBookManagerPreferences,
  type BookManagerBackgroundImage,
  type BookManagerPreferences,
  type BookManagerSettingTabKey,
  type BookManagerTxtParser
} from '../utils/bookManagerPreferences'
import UserDAL from '../user/userdal'
import type { ITokenInfo } from '../user/userstore'
import BookReaderModal from './BookReaderModal.vue'
import StatsPage from './StatsPage.vue'
import { useReaderI18n } from '../utils/readerI18n'

const bookStore = useBookLibraryStore()
const query = ref('')
const selectedBook = ref<IBookItem | null>(null)
const pendingAnnotationTarget = ref<{ type: 'note' | 'highlight' | 'bookmark'; id: string; action?: 'show' | 'edit'; requestId: number } | null>(null)
const groupDetail = ref<{ type: 'author' | 'format' | 'folder' | 'shelf'; title: string; items: IBookItem[] } | null>(null)
const readerVisible = ref(false)
type BookFolderGroup = ReturnType<typeof useBookLibraryStore>['byFolder'][number]
const userLabelMap = ref<Record<string, string>>({})
const scanAccounts = ref<ITokenInfo[]>([])
const selectedScanUserIds = ref<string[]>([])
const selectedFolderKeys = ref<string[]>([])
const selectedBookIds = ref<string[]>([])
const selectedAnnotationTags = ref<string[]>([])
const selectedAnnotationBookId = ref('')
const annotationSortMode = ref<BookAnnotationSortMode>('date')
const annotationSortOrder = ref<BookManagerSortOrder>('desc')
const folderContextVisible = ref(false)
const folderContextPosition = ref({ x: 0, y: 0 })
const folderContextGroup = ref<BookFolderGroup | null>(null)
const bookContextVisible = ref(false)
const bookContextPosition = ref({ x: 0, y: 0 })
const bookContextBook = ref<IBookItem | null>(null)
const activeManagerView = ref<BookManagerView>('home')
const readingStatusFilter = ref<ReadingStatus>('')
const sidebarCollapsed = ref(false)
const cardScale = ref(1)
const newTagName = ref('')
const editingTag = ref('')
const editingTagValue = ref('')
const showTagEditor = ref(false)
const tagEditMode = ref(false)
const exportAllFormat = ref<AnnotationExportFormat>('md')
const brokenCoverImages = ref<Set<string>>(new Set())
const showManagerSettings = ref(false)
const activeSettingTab = ref<BookManagerSettingTabKey>('appearance')
const managerPreferences = ref<BookManagerPreferences>(loadBookManagerPreferences())
const customColorDraft = ref(managerPreferences.value.themeColor === 'default' ? '#399393' : managerPreferences.value.themeColor)
const txtParserDraft = ref<BookManagerTxtParser>({ label: '', regex: '' })
const dictionaryDraftName = ref('')
const localFileInput = ref<HTMLInputElement>()

const softwareProtectionEnabled = ref(false)
const softwareProtectionMethod = ref('password')

const readerLocale = ref(localStorage.getItem('reader.locale') || 'zh')
function setReaderLocale(value: string) {
  readerLocale.value = value
  localStorage.setItem('reader.locale', value)
  const { setLocale } = useReaderI18n()
  setLocale(value === 'en' ? 'en' : 'zh')
}

function onCoverImgError(bookId: string, event: Event) {
  (event.target as HTMLImageElement).style.display = 'none'
  brokenCoverImages.value = new Set(brokenCoverImages.value).add(bookId)
}

function isCoverBroken(bookId: string): boolean {
  return brokenCoverImages.value.has(bookId)
}

const readerTabs = getBookManagerTabs()
const readerTabIcons: Partial<Record<BookManagerView, any>> = {
  home: BookMarked,
  recent: RefreshCw,
  favorites: Star,
  shelves: LibraryBig,
  notes: StickyNote,
  highlights: Highlighter,
  bookmarks: Bookmark,
  trash: Trash2,
  folders: Folder,
  formats: FileText,
  stats: BarChart3
}

const settingTabIcons: Record<BookManagerSettingTabKey, any> = {
  general: Settings,
  background: Settings,
  data: Archive,
  reading: BookOpen,
  appearance: Palette,
  more: MoreVertical,
  plugins: Globe2,
  ai: Lightbulb,
  txt: Languages,
  dict: Database,
  about: Info,
}

const settingSwitchGroups: Record<BookManagerSettingTabKey, Array<{ key: keyof BookManagerPreferences; label: string; desc?: string; enabled?: boolean }>> = {
  background: [],
  general: [
    { key: 'isImportPath', label: '导入书籍为链接', desc: '只保存原始路径，不复制书籍文件。当前网盘书库会保存该偏好，后续本地导入使用。' },
    { key: 'isDisableTrashBin', label: '禁用回收站', desc: '删除书籍库记录时直接永久删除本地记录。' },
    { key: 'isHideShelfBook', label: '隐藏已加入书架的书籍', desc: '首页不再显示已经分配到书架的书籍。' },
    { key: 'isDeleteShelfBook', label: '从书架删除时也删除书籍', desc: '保存 Koodo 行为偏好，后续书架删除流程复用。' },
    { key: 'isPreventSleep', label: '禁用屏幕休眠' },
    { key: 'isAlwaysOnTop', label: '窗口置顶' },
    { key: 'isAutoMaximizeWin', label: '启动时自动最大化主窗口' },
    { key: 'isAutoLaunch', label: '开机自动启动' },
    { key: 'isMinimizeToTray', label: '关闭时最小化到托盘' },
    { key: 'isPreventAdd', label: '打开书籍时不加入资料库' },
    { key: 'isPrecacheBook', label: '导入后自动预缓存书籍' },
    { key: 'isExportOriginalName', label: '导出书籍时使用原始文件名' },
    { key: 'isDisableAI', label: '禁用 AI 功能', desc: '仅影响书籍页入口偏好，已有全局 AI 配置仍保留。' },
    { key: 'isUseOriginalName', label: '使用文件名作为书名' },
    { key: 'isDisableUpdate', label: '禁用更新通知' },
    { key: 'isDeleteOriginal', label: '永久删除时删除原始文件', desc: '当前网盘文件不会被删除，只保存该 Koodo 设置偏好。' },
    { key: 'isUseBuiltIn', label: '使用内置浏览器打开链接' },
  ],
  data: [
    { key: 'isEnableDiscordRPC', label: '启用 Discord Rich Presence', desc: '显示阅读状态偏好；当前不接 Discord 运行时。' },
  ],
  reading: [
    { key: 'isTouch', label: '开启触屏模式' },
    { key: 'isPreventTrigger', label: '防误触', desc: '保存 Koodo 阅读菜单触发偏好。' },
    { key: 'isMergeWord', label: '合并阅读器到 Word' },
    { key: 'isOpenInMain', label: '在主窗口打开书籍' },
    { key: 'isManualScroll', label: '禁用 AI 聊天自动滚到底部' },
    { key: 'isOpenBook', label: '启动时自动打开上次阅读的书' },
    { key: 'isAutoMaximize', label: '打开书籍时自动最大化' },
    { key: 'isAutoFullscreen', label: '打开书籍时自动全屏' },
    { key: 'isDisablePopup', label: '选中文字时不弹出菜单' },
    { key: 'isDisableAutoScroll', label: '禁用章节末尾自动翻章' },
    { key: 'isOverwriteLink', label: '覆盖书籍默认链接样式' },
    { key: 'isOverwriteText', label: '覆盖书籍默认文本样式' },
  ],
  appearance: [
    { key: 'isDisablePDFCover', label: '不使用 PDF 首页作为封面' },
    { key: 'isDisableCrop', label: '不裁剪图书封面' },
    { key: 'isShowShelfBookCount', label: '显示每个书架中的图书数量' },
    { key: 'isCustomSystemCSS', label: '自定义应用样式', desc: '使用 CSS 自定义整个书籍页外观。' },
  ],
  more: [],
  plugins: [],
  ai: [],
  txt: [],
  dict: [],
  about: [],
}

const systemFontOptions = [
  { value: 'Built-in font', label: '内置字体' },
  { value: 'system-ui', label: '系统默认' },
  { value: 'PingFang SC', label: '苹方' },
  { value: 'Microsoft YaHei', label: '微软雅黑' },
  { value: 'Songti SC', label: '宋体' },
  { value: 'Arial', label: 'Arial' },
  { value: 'Georgia', label: 'Georgia' },
  { value: 'serif', label: 'Serif' },
  { value: 'sans-serif', label: 'Sans Serif' },
] as const

const searchEngineOptions = [
  { value: 'google', label: 'Google' },
  { value: 'baidu', label: '百度' },
  { value: 'bing', label: 'Bing' },
  { value: 'duckduckgo', label: 'DuckDuckGo' },
  { value: 'yandex', label: 'Yandex' },
  { value: 'yahoo', label: 'Yahoo' },
  { value: 'naver', label: 'Naver' },
  { value: 'baiduBaike', label: '百度百科' },
  { value: 'wiki', label: 'Wikipedia' },
] as const

const skinOptions = [
  { value: 'system', label: '追随系统' },
  { value: 'light', label: '白天模式' },
  { value: 'night', label: '黑夜模式' },
] as const

const managerAppearanceClass = computed(() => {
  return [
    managerPreferences.value.isDisableCrop ? 'cover-no-crop' : '',
    managerPreferences.value.isCustomSystemCSS ? 'custom-css-enabled' : '',
  ]
})

const managerAppearanceStyle = computed<CSSProperties>(() => {
  const prefs = managerPreferences.value
  const selectedBackground = prefs.backgroundImages.find((item) => item.id === prefs.appBackgroundImageId)
  const theme = BOOK_MANAGER_THEME_COLORS.find((item) => item.value === prefs.themeColor)
  const primary = prefs.themeColor === 'default' ? '#399393' : theme?.color || prefs.themeColor
  return {
    '--book-manager-primary': primary,
    '--book-manager-font-family': prefs.systemFont === 'Built-in font' ? 'inherit' : prefs.systemFont,
    '--book-manager-background-image': selectedBackground ? `url("${selectedBackground.dataUrl}")` : 'none',
  } as CSSProperties
})

const activeSettingTitle = computed(() => {
  return BOOK_MANAGER_SETTING_TABS.find((tab) => tab.key === activeSettingTab.value)?.label || '设置'
})

const hasAnyBookRecords = computed(() => bookStore.totalCount + bookStore.deletedCount > 0)

const searchedBooks = computed(() => {
  const q = query.value.trim().toLowerCase()
  const source = activeManagerView.value === 'home' && managerPreferences.value.isHideShelfBook
    ? bookStore.activeBooks.filter((book) => !book.shelf_id)
    : bookStore.activeBooks
  if (!q) return source
  return source.filter((b) => {
    return [
      b.title,
      b.author,
      b.file_name,
      b.parent_path,
      b.ext
    ].some((v) => String(v || '').toLowerCase().includes(q))
  })
})

function managerListView(view: BookManagerView): Exclude<BookManagerView, 'shelf' | 'folder' | 'format'> {
  switch (view) {
    case 'home':
    case 'recent':
    case 'favorites':
    case 'notes':
    case 'highlights':
    case 'bookmarks':
    case 'trash':
    case 'stats':
      return view
    case 'shelves':
    case 'folders':
    case 'formats':
    case 'shelf':
    case 'folder':
    case 'format':
      return 'home'
    default: {
      const exhaustive: never = view
      return exhaustive
    }
  }
}

const effectiveSortMode = computed<BookManagerSortMode>(() => activeManagerView.value === 'recent' ? 'recent' : bookStore.sortMode)
const effectiveSortOrder = computed<BookManagerSortOrder>(() => activeManagerView.value === 'recent' ? 'desc' : bookStore.sortOrder)

const readerVisibleBooks = computed(() => {
  const statusFiltered = filterBooksByReadingStatus(searchedBooks.value, readingStatusFilter.value)
  const filtered = filterBooksForManagerView(statusFiltered, { view: managerListView(activeManagerView.value) })
  return sortBooksForManagerView(filtered, effectiveSortMode.value, effectiveSortOrder.value)
})

const trashVisibleBooks = computed(() => {
  const q = query.value.trim().toLowerCase()
  const items = q
    ? bookStore.deletedBooks.filter((b) => [
      b.title,
      b.author,
      b.file_name,
      b.parent_path,
      b.ext
    ].some((v) => String(v || '').toLowerCase().includes(q)))
    : bookStore.deletedBooks
  return sortBooksForManagerView(items, bookStore.sortMode, bookStore.sortOrder)
})

const detailBooks = computed(() => {
  if (!groupDetail.value) return readerVisibleBooks.value
  const groupBookIds = new Set(groupDetail.value.items.map((book) => book.id))
  const currentGroupBooks = bookStore.activeBooks.filter((book) => groupBookIds.has(book.id))
  const q = query.value.trim().toLowerCase()
  const items = q
    ? currentGroupBooks.filter((b) => [
      b.title,
      b.author,
      b.file_name,
      b.parent_path,
      b.ext
    ].some((v) => String(v || '').toLowerCase().includes(q)))
    : currentGroupBooks
  return sortBooksForManagerView(items, effectiveSortMode.value, effectiveSortOrder.value)
})

const isCollectionManagerView = computed(() => {
  return ['home', 'recent', 'favorites'].includes(activeManagerView.value)
})

const isTrashManagerView = computed(() => activeManagerView.value === 'trash')

const currentAnnotationSource = computed<IBookNote[]>(() => activeManagerView.value === 'highlights' ? bookStore.allHighlights : bookStore.allNotes)
const annotationTagOptions = computed(() => getAnnotationTags(currentAnnotationSource.value))
const annotationBookOptions = computed(() => {
  const ids = new Set(currentAnnotationSource.value.map((note) => note.book_id))
  return bookStore.activeBooks
    .filter((book) => ids.has(book.id))
    .map((book) => ({ value: book.id, label: book.title || book.file_name }))
    .sort((a, b) => a.label.localeCompare(b.label))
})
const filteredNotes = computed(() => sortAnnotations(filterAnnotations(bookStore.allNotes, {
  tags: selectedAnnotationTags.value,
  bookId: selectedAnnotationBookId.value,
  keyword: query.value
}), annotationSortMode.value, annotationSortOrder.value))
const filteredHighlights = computed(() => sortAnnotations(filterAnnotations(bookStore.allHighlights, {
  tags: selectedAnnotationTags.value,
  bookId: selectedAnnotationBookId.value,
  keyword: query.value
}), annotationSortMode.value, annotationSortOrder.value))
const filteredBookmarks = computed(() => filterBookmarks(bookStore.allBookmarks, query.value))

const bookSearchPlaceholder = computed(() => {
  if (activeManagerView.value === 'notes') return '搜索我的笔记'
  if (activeManagerView.value === 'highlights') return '搜索我的书摘'
  if (activeManagerView.value === 'bookmarks') return '搜索我的书签'
  return '搜索书名、作者、格式'
})

const activeManagerTitle = computed(() => {
  if (groupDetail.value) return groupDetail.value.title
  return readerTabs.find((tab) => tab.key === activeManagerView.value)?.label || '全部图书'
})

const activeManagerSubtitle = computed(() => {
  if (bookStore.isScanning) return `${bookStore.scanLabel} · 已扫 ${bookStore.scanScanned} 项 · 命中 ${bookStore.scanFound} 本`
  if (activeManagerView.value === 'favorites') return '收藏的书会在这里集中展示'
  if (activeManagerView.value === 'recent') return '按最近阅读时间排列'
  if (activeManagerView.value === 'shelves') return '按书架整理和浏览藏书'
  if (activeManagerView.value === 'folders') return '按网盘来源文件夹浏览'
  if (activeManagerView.value === 'formats') return '按书籍格式浏览'
  if (activeManagerView.value === 'notes') return '按最近更新汇总带备注的读书笔记'
  if (activeManagerView.value === 'highlights') return '按最近更新汇总所有高亮书摘'
  if (activeManagerView.value === 'bookmarks') return '按最近更新汇总所有书签'
  if (activeManagerView.value === 'trash') return `${bookStore.deletedCount} 本已移入回收站的书籍库记录`
  return '从网盘里扫描 epub、mobi、pdf、txt、azw3 等常见书籍格式'
})

function formatTime(ts: number): string {
  if (!ts) return '从未扫描'
  const d = new Date(ts)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

const folderContextStyle = computed<CSSProperties>(() => ({
  position: 'fixed',
  left: `${folderContextPosition.value.x}px`,
  top: `${folderContextPosition.value.y}px`,
  zIndex: 9999,
  opacity: folderContextVisible.value ? 1 : 0
}))

const folderContextSelected = computed(() => {
  const group = folderContextGroup.value
  return !!group && selectedFolderKeys.value.includes(group.key)
})

const selectedFolderGroups = computed(() => {
  const keys = new Set(selectedFolderKeys.value)
  return bookStore.byFolder.filter((g) => keys.has(g.key))
})

const selectedBooks = computed(() => {
  const ids = new Set(selectedBookIds.value)
  return bookStore.books.filter((book) => ids.has(book.id))
})

const selectableBooks = computed(() => {
  if (groupDetail.value) return detailBooks.value
  if (isTrashManagerView.value) return trashVisibleBooks.value
  if (isCollectionManagerView.value) return readerVisibleBooks.value
  return []
})

const isAllSelectableBooksSelected = computed(() => {
  const ids = selectableBooks.value.map((book) => book.id)
  return ids.length > 0 && ids.every((id) => selectedBookIds.value.includes(id))
})

const scanAccountOptions = computed(() => {
  return scanAccounts.value
    .filter((u) => !!u?.user_id && !!u?.access_token)
    .map((u) => ({ value: u.user_id, label: scanAccountLabel(u) }))
})

async function openManagerView(tab: BookManagerView) {
  activeManagerView.value = tab
  groupDetail.value = null
  selectedBookIds.value = []
  if (tab === 'notes' || tab === 'highlights' || tab === 'bookmarks') {
    await bookStore.loadAllBookAnnotations()
    selectedAnnotationTags.value = selectedAnnotationTags.value.filter((tag) => annotationTagOptions.value.includes(tag))
    if (selectedAnnotationBookId.value && !annotationBookOptions.value.some((item) => item.value === selectedAnnotationBookId.value)) {
      selectedAnnotationBookId.value = ''
    }
  }
}

function toggleAnnotationTag(tag: string) {
  if (selectedAnnotationTags.value.includes(tag)) {
    selectedAnnotationTags.value = selectedAnnotationTags.value.filter((item) => item !== tag)
  } else {
    selectedAnnotationTags.value = [...selectedAnnotationTags.value, tag]
  }
}

function clearAnnotationFilters() {
  selectedAnnotationTags.value = []
  selectedAnnotationBookId.value = ''
}

function handleAddTag() {
  const tag = newTagName.value.trim()
  if (!tag) return
  bookStore.addNoteTag(tag)
  newTagName.value = ''
}

async function handleDeleteTag(tag: string) {
  await bookStore.removeNoteTag(tag)
  selectedAnnotationTags.value = selectedAnnotationTags.value.filter((item) => item !== tag)
}

function startRenameTag(tag: string) {
  editingTag.value = tag
  editingTagValue.value = tag
}

async function handleRenameTag() {
  const oldTag = editingTag.value
  const newTag = editingTagValue.value.trim()
  if (!newTag || oldTag === newTag || !oldTag) {
    cancelRenameTag()
    return
  }
  const count = await bookStore.renameNoteTag(oldTag, newTag)
  if (count > 0) {
    if (selectedAnnotationTags.value.includes(oldTag)) {
      selectedAnnotationTags.value = selectedAnnotationTags.value.map((item) => item === oldTag ? newTag : item)
    }
  }
  cancelRenameTag()
}

function cancelRenameTag() {
  editingTag.value = ''
  editingTagValue.value = ''
}

let tagEditKeyHandler: ((e: KeyboardEvent) => void) | null = null

function bindTagEditKeydown() {
  if (tagEditKeyHandler) return
  tagEditKeyHandler = (e: KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      handleRenameTag()
    } else if (e.key === 'Escape') {
      cancelRenameTag()
    }
  }
  document.addEventListener('keydown', tagEditKeyHandler)
}

function unbindTagEditKeydown() {
  if (tagEditKeyHandler) {
    document.removeEventListener('keydown', tagEditKeyHandler)
    tagEditKeyHandler = null
  }
}

function toggleTagEditor() {
  tagEditMode.value = !tagEditMode.value
  if (tagEditMode.value) {
    bindTagEditKeydown()
  } else {
    unbindTagEditKeydown()
    cancelRenameTag()
  }
}

function handleExportAllAnnotations() {
  const currentNotes = currentAnnotationSource.value
  const notes = sortAnnotations(filterAnnotations(currentNotes, {
    tags: selectedAnnotationTags.value,
    bookId: selectedAnnotationBookId.value,
    keyword: query.value
  }), annotationSortMode.value, annotationSortOrder.value)
  const bookmarks = filterBookmarks(bookStore.allBookmarks, query.value)
  if (!notes.length && !bookmarks.length) {
    message.warning('暂无可导出的书摘或书签')
    return
  }
  exportAllAnnotations({
    books: bookStore.activeBooks,
    notes,
    bookmarks
  }, exportAllFormat.value)
  message.success('已导出全部书摘')
}

function handleSortModeChange(value: unknown) {
  bookStore.setSortMode(value as BookManagerSortMode)
}

function toggleSortOrder() {
  bookStore.setSortOrder(bookStore.sortOrder === 'asc' ? 'desc' : 'asc')
}

const sortModeOptions = [
  { value: 'added', label: '最近添加' },
  { value: 'recent', label: '最近阅读' },
  { value: 'title', label: '书名' },
  { value: 'author', label: '作者' },
  { value: 'readingTime', label: '阅读时长' },
  { value: 'progress', label: '阅读进度' },
  { value: 'size', label: '文件大小' },
] as const

function sortModeLabel(mode: string) {
  return sortModeOptions.find((o) => o.value === mode)?.label ?? mode
}

const annotationSortModeOptions = [
  { value: 'date', label: '笔记时间' },
  { value: 'progress', label: '阅读进度' }
] as const

function handleAnnotationSortModeChange(value: Event) {
  annotationSortMode.value = (value.target as HTMLSelectElement).value as BookAnnotationSortMode
}

function toggleAnnotationSortOrder() {
  annotationSortOrder.value = annotationSortOrder.value === 'asc' ? 'desc' : 'asc'
}

function handleViewModeChange(value: unknown) {
  bookStore.setViewMode(value as BookViewMode)
}

function updateManagerPreferences(patch: Partial<BookManagerPreferences>) {
  managerPreferences.value = saveBookManagerPreferences({ ...managerPreferences.value, ...patch })
}

function setManagerPreferenceText(key: keyof BookManagerPreferences, value: string) {
  updateManagerPreferences({ [key]: value } as Partial<BookManagerPreferences>)
}

function handleManagerSelectChange(key: keyof BookManagerPreferences, event: Event) {
  setManagerPreferenceText(key, (event.target as HTMLSelectElement).value)
}

function toggleManagerPreference(key: keyof BookManagerPreferences) {
  const current = managerPreferences.value[key]
  if (typeof current !== 'boolean') return
  updateManagerPreferences({ [key]: !current } as Partial<BookManagerPreferences>)
}

function setThemeColor(value: string) {
  customColorDraft.value = value === 'default' ? customColorDraft.value : value
  updateManagerPreferences({ themeColor: value })
}

function setCustomThemeColor() {
  const value = customColorDraft.value.trim()
  if (!/^#[0-9a-fA-F]{6}$/.test(value)) {
    message.error('请输入 6 位十六进制颜色，例如 #399393')
    return
  }
  updateManagerPreferences({ themeColor: value })
}

function addTxtParser() {
  const label = txtParserDraft.value.label.trim()
  const regex = txtParserDraft.value.regex.trim()
  if (!label || !regex) {
    message.warning('请输入解析器名称和正则')
    return
  }
  try {
    new RegExp(regex)
  } catch {
    message.error('正则表达式无效')
    return
  }
  const next = managerPreferences.value.txtParsers.filter((item) => item.label !== label)
  updateManagerPreferences({ txtParsers: [...next, { label, regex }] })
  txtParserDraft.value = { label: '', regex: '' }
}

function deleteTxtParser(label: string) {
  updateManagerPreferences({ txtParsers: managerPreferences.value.txtParsers.filter((item) => item.label !== label) })
}

function addLocalDictionary() {
  const name = dictionaryDraftName.value.trim()
  if (!name) return
  updateManagerPreferences({
    localDictionaries: [
      ...managerPreferences.value.localDictionaries,
      { id: String(Date.now()), name, extension: 'mdx' }
    ]
  })
  dictionaryDraftName.value = ''
}

function deleteLocalDictionary(id: string) {
  updateManagerPreferences({ localDictionaries: managerPreferences.value.localDictionaries.filter((item) => item.id !== id) })
}

async function scanBooks() {
  if (bookStore.isScanning) return
  if (!selectedScanUserIds.value.length) {
    message.warning('请选择要扫描的网盘')
    return
  }
  await BookScanner.getInstance().scanAllUsers({
    userIdAllowList: new Set(selectedScanUserIds.value)
  })
  message.success(`书籍扫描完成，已收录 ${bookStore.totalCount} 本`)
}

function stopScan() {
  BookScanner.getInstance().stopScan()
}

function triggerLocalImport() {
  localFileInput.value?.click()
}

function openExternalLink(url: string) {
  window.open(url, '_blank')
}

async function copyContactEmail() {
  try {
    await navigator.clipboard.writeText('gaozhangmin@gmail.com')
    message.success('已复制邮箱')
  } catch {
    message.error('复制失败')
  }
}

async function handleLocalFileImport(event: Event) {
  const input = event.target as HTMLInputElement
  const files = input.files
  if (!files || !files.length) return

  const formatExtensions = ['epub', 'pdf', 'mobi', 'azw3', 'txt', 'azw', 'djvu', 'fb2', 'cbr', 'cbz', 'cbt', 'cb7', 'docx', 'md', 'markdown', 'html', 'htm']
  const validFiles: File[] = []

  for (let i = 0; i < files.length; i++) {
    const ext = files[i].name.split('.').pop()?.toLowerCase() || ''
    if (formatExtensions.includes(ext)) validFiles.push(files[i])
  }

  if (!validFiles.length) {
    message.warning('没有可导入的图书文件（支持 epub, pdf, mobi, azw3, txt 等）')
    input.value = ''
    return
  }

  const maxSize = 50 * 1024 * 1024 // 50MB limit
  const imported: IBookItem[] = []
  const now = Date.now()

  for (const file of validFiles) {
    if (file.size > maxSize) {
      message.warning(`《${file.name}》超过 50MB，已跳过`)
      continue
    }
    try {
      const ext = file.name.split('.').pop()?.toLowerCase() || ''
      const buffer = await readFileAsArrayBuffer(file)
      if (!buffer) continue
      const dataUrl = arrayBufferToDataUrl(buffer, ext)

      const bookId = `local-${now}-${Math.random().toString(36).slice(2, 8)}-${imported.length}`
      const nameMeta = parseBookMeta(file.name)

      // 从 EPUB 文件内容提取元数据
      let fileMeta: Pick<IBookItem, 'title' | 'author' | 'summary' | 'cover_url'> = {}
      if (ext === 'epub') {
        try {
          const { extractEpubMeta } = await import('../utils/bookEpubMeta')
          const epubMeta = await extractEpubMeta(buffer)
          if (epubMeta.title) fileMeta.title = epubMeta.title
          if (epubMeta.author) fileMeta.author = epubMeta.author
          if (epubMeta.description) fileMeta.summary = epubMeta.description
          if (epubMeta.coverDataUrl) fileMeta.cover_url = epubMeta.coverDataUrl
        } catch {}
      }

      const book: IBookItem = {
        id: bookId,
        user_id: 'local',
        drive_id: 'local',
        file_id: bookId,
        parent_file_id: 'local_root',
        file_name: file.name,
        ext,
        size: file.size,
        category: extractCategoryFromExt(ext),
        title: fileMeta.title || nameMeta.title || file.name.replace(/\.[^.]+$/, ''),
        author: fileMeta.author || nameMeta.author || '未知作者',
        summary: fileMeta.summary || '',
        cover_url: fileMeta.cover_url || '',
        description: JSON.stringify({ imported: true, dataUrl }),
        scanned_at: now,
        updated_at: now,
        metadata_source: ext === 'epub' && fileMeta.title ? 'epub_metadata' : 'local'
      }
      imported.push(book)
    } catch (e: any) {
      console.error(`Failed to import ${file.name}:`, e)
    }
  }

  if (imported.length) {
    await bookStore.appendBooks(imported)
    message.success(`成功导入 ${imported.length} 本图书`)
  } else {
    message.warning('导入失败，请检查文件是否可读')
  }

  input.value = ''
}

function readFileAsArrayBuffer(file: File): Promise<ArrayBuffer | null> {
  return new Promise((resolve) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as ArrayBuffer)
    reader.onerror = () => resolve(null)
    reader.readAsArrayBuffer(file)
  })
}

function arrayBufferToDataUrl(buffer: ArrayBuffer, ext: string): string {
  const mimeMap: Record<string, string> = {
    epub: 'application/epub+zip',
    pdf: 'application/pdf',
    mobi: 'application/x-mobipocket-ebook',
    azw3: 'application/vnd.amazon.mobi8-ebook',
    azw: 'application/vnd.amazon.ebook',
    txt: 'text/plain',
    fb2: 'application/x-fictionbook+xml',
    cbr: 'application/vnd.comicbook-rar',
    cbz: 'application/vnd.comicbook+zip',
    cbt: 'application/x-cbt',
    cb7: 'application/x-cb7',
    docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    md: 'text/markdown',
    html: 'text/html',
  }
  const mime = mimeMap[ext] || 'application/octet-stream'
  const bytes = new Uint8Array(buffer)
  let binary = ''
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i])
  }
  return `data:${mime};base64,${btoa(binary)}`
}

function extractCategoryFromExt(ext: string): string {
  const catMap: Record<string, string> = {
    epub: 'book', mobi: 'book', azw3: 'book', azw: 'book', fb2: 'book', txt: 'book',
    pdf: 'document', docx: 'document',
    md: 'document', markdown: 'document', html: 'document', htm: 'document',
    cbr: 'comic', cbz: 'comic', cbt: 'comic', cb7: 'comic',
    djvu: 'book',
  }
  return catMap[ext] || 'book'
}

async function clearLibrary() {
  await bookStore.clearAll()
  selectedBook.value = null
  groupDetail.value = null
  message.success('书籍库已清空')
}

function openBook(book: IBookItem, options: { keepAnnotationTarget?: boolean } = {}) {
  if (!options.keepAnnotationTarget) pendingAnnotationTarget.value = null

  if (window.WebOpenWindow) {
    window.WebOpenWindow({ page: 'PageBookReader', data: JSON.parse(JSON.stringify(book)), theme: 'dark' })
    return
  }

  selectedBook.value = book
  readerVisible.value = true
}

function readerBadgeFor(book: IBookItem): { label: string; tone: 'native' | 'reader' | 'legacy' } | null {
  const ext = book.ext || ''
  if (isReaderFormat(ext)) return { label: '内置阅读', tone: 'reader' }
  if (isLegacyScanOnlyBookFormat(ext)) return { label: '暂不支持阅读', tone: 'legacy' }
  return null
}

function readingProgressLabel(book: IBookItem): string {
  if (typeof book.reading_progress === 'number') {
    if (book.reading_progress === 0) return '新书'
    if (book.reading_progress >= 100) return '已读完'
    return `${book.reading_progress.toFixed(1)}%`
  }
  if (book.reading_progress_text) return book.reading_progress_text
  return ''
}

function bookForAnnotation(annotation: Pick<IBookNote | IBookBookmark, 'book_id'>): IBookItem | undefined {
  return bookStore.activeBooks.find((book) => book.id === annotation.book_id)
}

function annotationBookTitle(annotation: Pick<IBookNote | IBookBookmark, 'book_id'>): string {
  const book = bookForAnnotation(annotation)
  return book?.title || book?.file_name || '未知书籍'
}

function openAnnotationBook(annotation: Pick<IBookNote | IBookBookmark, 'book_id' | 'id'>, type: 'note' | 'highlight' | 'bookmark', action: 'show' | 'edit' = 'show') {
  const book = bookForAnnotation(annotation)
  pendingAnnotationTarget.value = { type, id: annotation.id, action, requestId: Date.now() }
  if (book) openBook(book, { keepAnnotationTarget: true })
}

function copyAnnotationLink(annotation: Pick<IBookNote, 'id'>, event?: Event) {
  event?.stopPropagation()
  copyToClipboard(buildNoteLink(annotation.id))
  message.success('笔记链接已复制')
}

async function deleteAnnotation(annotation: Pick<IBookNote, 'book_id' | 'id'>, event?: Event) {
  event?.stopPropagation()
  const { book_id, id } = annotation
  if (!book_id || !id) return
  await bookStore.deleteBookNotesByIds(book_id, [id])
  message.success('已删除')
}

function editAnnotationInBook(annotation: Pick<IBookNote, 'book_id' | 'id'>, type: 'note' | 'highlight', event?: Event) {
  event?.stopPropagation()
  openAnnotationBook(annotation, type, 'edit')
}

function showAnnotationInBook(annotation: Pick<IBookNote | IBookBookmark, 'book_id' | 'id'>, type: 'note' | 'highlight' | 'bookmark', event?: Event) {
  event?.stopPropagation()
  openAnnotationBook(annotation, type)
}

function bookmarkPercentLabel(bookmark: Pick<IBookBookmark, 'percentage'>): string {
  const percent = Number(bookmark.percentage)
  if (!Number.isFinite(percent)) return '-'
  return `${Math.max(0, Math.min(100, Math.round(percent * 100)))}%`
}

function putBackBook() {
  selectedBook.value = null
}

function openGroupDetail(type: 'author' | 'format' | 'folder' | 'shelf', title: string, items: IBookItem[]) {
  selectedBookIds.value = []
  groupDetail.value = { type, title, items }
  nextTick(() => document.querySelector('.book-content')?.scrollTo({ top: 0, behavior: 'smooth' }))
}

function isBookSelected(book: IBookItem): boolean {
  return selectedBookIds.value.includes(book.id)
}

function toggleBookSelected(book: IBookItem, event?: Event) {
  event?.stopPropagation()
  selectedBookIds.value = selectedBookIds.value.includes(book.id)
    ? selectedBookIds.value.filter((id) => id !== book.id)
    : [...selectedBookIds.value, book.id]
}

function clearSelectedBooks() {
  selectedBookIds.value = []
}

function toggleSelectAllVisibleBooks() {
  const ids = selectableBooks.value.map((book) => book.id)
  if (!ids.length) return
  if (isAllSelectableBooksSelected.value) {
    const visibleSet = new Set(ids)
    selectedBookIds.value = selectedBookIds.value.filter((id) => !visibleSet.has(id))
  } else {
    selectedBookIds.value = Array.from(new Set([...selectedBookIds.value, ...ids]))
  }
}

async function assignSelectedBooksToShelf() {
  if (!selectedBookIds.value.length) return
  const raw = window.prompt('输入书架名称', '')
  if (raw === null) return
  const shelfName = buildDefaultShelfName(raw)
  const shelfId = shelfName === '默认书架' ? '' : shelfName
  await bookStore.moveBooksToShelf(selectedBookIds.value, shelfId)
  if (selectedBook.value) selectedBook.value = bookStore.books.find((book) => book.id === selectedBook.value?.id) || selectedBook.value
  message.success(`已将 ${selectedBookIds.value.length} 本书加入 ${shelfName}`)
  clearSelectedBooks()
}

async function setSelectedBooksFavorite(isFavorite: boolean) {
  if (!selectedBookIds.value.length) return
  await Promise.all(selectedBooks.value.map((book) => bookStore.updateBookMetadata(book.id, { is_favorite: isFavorite })))
  message.success(isFavorite ? '已收藏所选书籍' : '已取消收藏所选书籍')
  clearSelectedBooks()
}

async function deleteSelectedBooks() {
  if (!selectedBookIds.value.length) return
  const ids = [...selectedBookIds.value]
  const permanently = managerPreferences.value.isDisableTrashBin
  const deleteOriginal = managerPreferences.value.isDeleteOriginal
  const ok = window.confirm(permanently
    ? `确定永久删除 ${ids.length} 本书籍库记录？${deleteOriginal ? '将同时尝试删除网盘原始文件。' : '不会删除网盘文件，但记录无法从回收站恢复。'}`
    : `确定将 ${ids.length} 本书籍库记录移入已删除？不会删除网盘文件。`)
  if (!ok) return
  if (permanently && deleteOriginal) {
    await deleteOriginalFiles(ids)
  }
  if (permanently) await bookStore.deleteBooksByIds(ids)
  else await bookStore.moveBooksToTrash(ids)
  if (selectedBook.value && ids.includes(selectedBook.value.id)) selectedBook.value = null
  message.success(permanently ? `已永久删除 ${ids.length} 本` : `已移入已删除 ${ids.length} 本`)
  clearSelectedBooks()
}

async function deleteOriginalFiles(ids: string[]) {
  const books = bookStore.books.filter((b) => ids.includes(b.id))
  for (const book of books) {
    try {
      const { default: AliFileCmd } = await import('../aliapi/filecmd')
      await AliFileCmd.ApiDeleteBatch(book.user_id, book.drive_id, [book.file_id]).catch(() => {})
    } catch { /* 删除网盘文件失败不影响本地删除 */ }
  }
}

async function restoreSelectedBooks() {
  if (!selectedBookIds.value.length) return
  const ids = [...selectedBookIds.value]
  await bookStore.restoreBooksFromTrash(ids)
  message.success(`已恢复 ${ids.length} 本书籍库记录`)
  clearSelectedBooks()
}

async function permanentlyDeleteSelectedBooks() {
  if (!selectedBookIds.value.length) return
  const ids = [...selectedBookIds.value]
  const deleteOriginal = managerPreferences.value.isDeleteOriginal
  const ok = window.confirm(`确定永久删除 ${ids.length} 本书籍库记录？${deleteOriginal ? '将同时尝试删除网盘原始文件。' : '不会删除网盘文件，但记录无法从回收站恢复。'}`)
  if (!ok) return
  if (deleteOriginal) await deleteOriginalFiles(ids)
  await bookStore.deleteBooksByIds(ids)
  message.success(`已永久删除 ${ids.length} 本书籍库记录`)
  clearSelectedBooks()
}

async function restoreBook(book: IBookItem, event?: Event) {
  event?.stopPropagation()
  await bookStore.restoreBooksFromTrash([book.id])
  selectedBookIds.value = selectedBookIds.value.filter((id) => id !== book.id)
  message.success('已恢复书籍记录')
}

async function permanentlyDeleteBook(book: IBookItem, event?: Event) {
  event?.stopPropagation()
  const deleteOriginal = managerPreferences.value.isDeleteOriginal
  const ok = window.confirm(`确定永久删除《${book.title || book.file_name}》的书籍库记录？${deleteOriginal ? '将同时尝试删除网盘原始文件。' : '不会删除网盘文件。'}`)
  if (!ok) return
  if (deleteOriginal) await deleteOriginalFiles([book.id])
  await bookStore.deleteBooksByIds([book.id])
  selectedBookIds.value = selectedBookIds.value.filter((id) => id !== book.id)
  message.success('已永久删除书籍记录')
}

async function clearDeletedBooks() {
  const ids = bookStore.deletedBooks.map((book) => book.id)
  if (!ids.length) return
  const ok = window.confirm(`确定清空已删除中的 ${ids.length} 本书籍库记录？不会删除网盘文件，但会清除这些书的本地笔记、书签和书摘。`)
  if (!ok) return
  await bookStore.deleteBooksByIds(ids)
  clearSelectedBooks()
  message.success(`已清空已删除 ${ids.length} 本`)
}

function booksForShelf(shelf: BookShelfGroup): IBookItem[] {
  const ids = new Set(shelf.book_ids)
  return bookStore.activeBooks.filter((book) => ids.has(book.id))
}

function openShelf(shelf: BookShelfGroup) {
  openGroupDetail('shelf', shelf.name, booksForShelf(shelf))
}

function shelfNameForBook(book: IBookItem): string {
  return book.shelf_id || '默认书架'
}

async function assignSelectedBookToShelf() {
  const book = selectedBook.value
  if (!book) return
  const raw = window.prompt('输入书架名称', book.shelf_id || '')
  if (raw === null) return
  const shelfName = buildDefaultShelfName(raw)
  const shelfId = shelfName === '默认书架' ? '' : shelfName
  await bookStore.moveBooksToShelf([book.id], shelfId)
  selectedBook.value = bookStore.books.find((item) => item.id === book.id) || selectedBook.value
  message.success(`已加入 ${shelfName}`)
}

async function removeSelectedBookFromShelf() {
  const book = selectedBook.value
  if (!book || !book.shelf_id) return
  if (managerPreferences.value.isDeleteShelfBook) {
    await bookStore.deleteBooksByIds([book.id])
    selectedBook.value = null
    message.success('已从书架移除并删除书籍')
  } else {
    await bookStore.moveBooksToShelf([book.id], '')
    selectedBook.value = bookStore.books.find((item) => item.id === book.id) || selectedBook.value
    message.success('已移回默认书架')
  }
}

function toggleBookFavorite(book: IBookItem, event: Event) {
  event.stopPropagation()
  bookStore.toggleFavoriteBook(book.id)
}

function driveLabel(driveId: string, userId = ''): string {
  if (driveId === 'cloud123') return '123 网盘'
  if (driveId === 'drive115') return '115 网盘'
  if (driveId === 'cloud139') return '139 云盘'
  if (driveId === 'cloud189') return '天翼云盘'
  if (driveId === 'baidu') return '百度网盘'
  if (driveId === 'pikpak') return 'PikPak'
  if (driveId === 'quark') return '夸克网盘'
  if (driveId === 'dropbox') return 'Dropbox'
  if (driveId === 'onedrive') return 'OneDrive'
  if (driveId === 'box') return 'Box'
  if (driveId.includes('resource')) return '阿里云盘资源盘'
  if (driveId.includes('backup')) return '阿里云盘备份盘'
  if (userId.startsWith('aliyun_') || driveId) return '阿里云盘'
  return '未知网盘'
}

function driveLabelFromToken(token: ITokenInfo): string {
  if (token.tokenfrom === 'cloud123') return '123 网盘'
  if (token.tokenfrom === '115') return '115 网盘'
  if (token.tokenfrom === '139') return '139 云盘'
  if (token.tokenfrom === '189') return '天翼云盘'
  if (token.tokenfrom === 'baidu') return '百度网盘'
  if (token.tokenfrom === 'pikpak') return 'PikPak'
  if (token.tokenfrom === 'quark') return '夸克网盘'
  if (token.tokenfrom === 'dropbox') return 'Dropbox'
  if (token.tokenfrom === 'onedrive') return 'OneDrive'
  if (token.tokenfrom === 'box') return 'Box'
  return '阿里云盘'
}

function scanAccountLabel(token: ITokenInfo): string {
  return `${driveLabelFromToken(token)} · ${token.nick_name || token.user_name || token.name || token.user_id}`
}

function userLabel(userId: string): string {
  return userLabelMap.value[userId] || userId || '未知账号'
}

function sourceLabel(book?: IBookItem): string {
  if (!book) return ''
  return `${driveLabel(book.drive_id, book.user_id)} · ${userLabel(book.user_id)}`
}

function folderSourceLabel(g: { user_id?: string; drive_id?: string }): string {
  return `${driveLabel(g.drive_id || '', g.user_id || '')} · ${userLabel(g.user_id || '')}`
}

function toggleFolderSelected(g: BookFolderGroup) {
  const set = new Set(selectedFolderKeys.value)
  if (set.has(g.key)) set.delete(g.key)
  else set.add(g.key)
  selectedFolderKeys.value = Array.from(set)
}

function openFolderCard(g: BookFolderGroup) {
  if (selectedFolderKeys.value.length > 0) {
    toggleFolderSelected(g)
    return
  }
  openGroupDetail('folder', g.name, g.items)
}

function openFolderContextMenu(event: MouseEvent, g: BookFolderGroup) {
  event.preventDefault()
  event.stopPropagation()
  folderContextGroup.value = g
  folderContextPosition.value = { x: event.clientX, y: event.clientY }
  folderContextVisible.value = true
}

function closeFolderContextMenu() {
  folderContextVisible.value = false
  folderContextGroup.value = null
}

function openBookContextMenu(event: MouseEvent, book: IBookItem) {
  bookContextBook.value = book
  bookContextPosition.value = { x: event.clientX, y: event.clientY }
  bookContextVisible.value = true
}

function closeBookContextMenu() {
  bookContextVisible.value = false
  bookContextBook.value = null
}

function bookContextStyle(): CSSProperties {
  return {
    position: 'fixed',
    left: `${bookContextPosition.value.x}px`,
    top: `${bookContextPosition.value.y}px`,
    zIndex: 9999,
    opacity: bookContextVisible.value ? 1 : 0
  }
}

async function bookContextToggleFavorite() {
  const b = bookContextBook.value
  closeBookContextMenu()
  if (!b) return
  await bookStore.toggleFavoriteBook(b.id)
}

function bookContextMoveToShelf() {
  const b = bookContextBook.value
  closeBookContextMenu()
  if (!b) return
  const name = window.prompt('输入书架名称', b.shelf_id || '')
  if (!name) return
  bookStore.moveBooksToShelf([b.id], name)
}

async function bookContextRemoveFromShelf() {
  const b = bookContextBook.value
  closeBookContextMenu()
  if (!b || !b.shelf_id) return
  if (managerPreferences.value.isDeleteShelfBook) {
    await bookStore.deleteBooksByIds([b.id])
    message.success('已从书架移除并删除书籍')
  } else {
    await bookStore.moveBooksToShelf([b.id], '')
    message.success('已移回默认书架')
  }
}

async function bookContextDelete() {
  const b = bookContextBook.value
  closeBookContextMenu()
  if (!b) return
  const permanently = managerPreferences.value.isDisableTrashBin
  const deleteOriginal = managerPreferences.value.isDeleteOriginal
  const ok = window.confirm(permanently
    ? `确定永久删除《${b.title || b.file_name}》的书籍库记录？${deleteOriginal ? '将同时尝试删除网盘原始文件。' : '不会删除网盘文件。'}`
    : `确定将《${b.title || b.file_name}》移入已删除？不会删除网盘文件。`)
  if (!ok) return
  if (permanently) {
    if (deleteOriginal) await deleteOriginalFiles([b.id])
    await bookStore.deleteBooksByIds([b.id])
  }
  else await bookStore.moveBooksToTrash([b.id])
  if (selectedBook.value?.id === b.id) selectedBook.value = null
  message.success(permanently ? '已永久删除书籍记录' : '已移入已删除')
}

function copyBookLink(book: IBookItem) {
  closeBookContextMenu()
  copyToClipboard(buildNoteLink(book.id))
  message.success('书籍链接已复制')
}

async function deleteFolderGroups(groups: BookFolderGroup[]) {
  const uniq = new Set<string>()
  let bookCount = 0
  for (const g of groups) {
    bookCount += g.items.length
    for (const b of g.items) uniq.add(b.id)
  }
  if (!uniq.size) return
  const permanently = managerPreferences.value.isDisableTrashBin
  const ok = window.confirm(permanently
    ? `确定永久删除 ${groups.length} 个文件夹的书籍库记录？将移除 ${bookCount} 本已刮削书籍，不会删除网盘文件。`
    : `确定将 ${groups.length} 个文件夹的书籍库记录移入已删除？将移除 ${bookCount} 本已刮削书籍，不会删除网盘文件。`)
  if (!ok) return
  if (permanently) await bookStore.deleteBooksByIds(Array.from(uniq))
  else await bookStore.moveBooksToTrash(Array.from(uniq))
  if (selectedBook.value && uniq.has(selectedBook.value.id)) selectedBook.value = null
  selectedFolderKeys.value = selectedFolderKeys.value.filter((key) => bookStore.byFolder.some((g) => g.key === key))
  if (groupDetail.value?.type === 'folder' && groups.some((g) => g.items.some((b) => groupDetail.value?.items.some((gb) => gb.id === b.id)))) {
    groupDetail.value = null
  }
  message.success(permanently ? `已永久删除 ${bookCount} 本书籍库记录` : `已移入已删除 ${bookCount} 本书籍库记录`)
}

function deleteCurrentFolderFromMenu() {
  const g = folderContextGroup.value
  closeFolderContextMenu()
  if (!g) return
  deleteFolderGroups([g])
}

function deleteSelectedFoldersFromMenu() {
  const groups = selectedFolderGroups.value
  closeFolderContextMenu()
  if (!groups.length) return
  deleteFolderGroups(groups)
}

function toggleContextFolderSelected() {
  const g = folderContextGroup.value
  if (g) toggleFolderSelected(g)
  closeFolderContextMenu()
}

function formatCoverColor(book?: IBookItem | null): string {
  if (!book) return '#6867d1'
  const fmt = (book.ext || '').toUpperCase()
  const colors: Record<string, string> = {
    PDF: 'rgba(55, 170, 81, 0.7)',
    TXT: 'rgba(251, 191, 16, 1)',
    EPUB: 'rgba(33, 165, 241, 1)',
    MOBI: 'rgba(255, 108, 110, 1)',
    AZW: '#ff9900',
    AZW3: '#ff9900',
    MD: '#5e7fff',
    MARKDOWN: '#5e7fff',
    FB2: '#0063b1',
    DOCX: '#6867d1',
    CBT: '#00b6c2',
    CBZ: '#00b6c2',
    CB7: '#00b6c2',
    CBR: '#00b6c2',
    HTML: '#e67e22',
    HTM: '#e67e22',
  }
  return colors[fmt] || '#6867d1'
}

function emptyCoverTitle(book: IBookItem): string {
  return bookDisplayTitle(book)
}

function bookDisplayTitle(book: IBookItem): string {
  if (managerPreferences.value.isUseOriginalName) return book.file_name || book.title || '未命名书籍'
  return book.title || book.file_name || '未命名书籍'
}

function shouldUseCoverImage(book: IBookItem): boolean {
  if (isCoverBroken(book.id)) return false
  if (managerPreferences.value.isDisablePDFCover && (book.ext || '').toLowerCase() === 'pdf') return false
  return !!(book.cover_url || book.thumbnail)
}

function bookCoverImage(book: IBookItem): string {
  return shouldUseCoverImage(book) ? String(book.cover_url || book.thumbnail || '') : ''
}

function shouldShowCoverFallback(book: IBookItem): boolean {
  return !shouldUseCoverImage(book)
}

onMounted(async () => {
  await bookStore.loadFromDB()
  const users = await UserDAL.GetUserListFromDB().catch(() => [])
  scanAccounts.value = users.filter((u) => !!u?.user_id && !!u?.access_token)
  selectedScanUserIds.value = scanAccounts.value.map((u) => u.user_id)
  const map: Record<string, string> = {}
  for (const u of users) {
    if (!u?.user_id) continue
    map[u.user_id] = u.nick_name || u.user_name || u.name || u.user_id
  }
  userLabelMap.value = map

  // 自动切换到指定书架 (shelf feature not yet implemented)

  // 自动打开上次阅读的书
  if (managerPreferences.value.isOpenBook) {
    const lastBook = bookStore.activeBooks.find((b) => b.last_read_at)
    if (lastBook) openBook(lastBook)
  }
})

// 动态注入自定义 CSS（Vue 编译器禁止 <style> 出现在 template 中）
let customStyleEl: HTMLStyleElement | null = null
watch(
  () => managerPreferences.value.isCustomSystemCSS && managerPreferences.value.customSystemCSS,
  (css) => {
    if (css) {
      if (!customStyleEl) {
        customStyleEl = document.createElement('style')
        customStyleEl.setAttribute('data-book-custom-css', '')
        document.head.appendChild(customStyleEl)
      }
      customStyleEl.textContent = css
    } else {
      if (customStyleEl) {
        customStyleEl.remove()
        customStyleEl = null
      }
    }
  },
  { immediate: true }
)
onUnmounted(() => {
  if (customStyleEl) {
    customStyleEl.remove()
    customStyleEl = null
  }
})

// 禁用屏幕休眠
watch(() => managerPreferences.value.isPreventSleep, (enabled) => {
  try { window.Electron?.ipcRenderer?.send('setPowerSaveBlocker', enabled) } catch {}
}, { immediate: true })
</script>

<template>
  <a-layout :class="['book-library', ...managerAppearanceClass]" :style='managerAppearanceStyle'>
    <aside :class="['book-sidebar', sidebarCollapsed ? 'collapsed' : '']">
      <div class='book-brand'>
        <button class='book-sidebar-toggle' :title="sidebarCollapsed ? '展开侧栏' : '折叠侧栏'" @click='sidebarCollapsed = !sidebarCollapsed'>
          <PanelLeftClose v-if='!sidebarCollapsed' :size='18' :stroke-width='2' />
          <PanelLeft v-else :size='18' :stroke-width='2' />
        </button>
        <LibraryBig :size='24' :stroke-width='1.7' />
        <div class='book-brand-text'>
          <div class='book-brand-title'>书籍</div>
          <div class='book-brand-sub'>{{ bookStore.totalCount }} 本藏书</div>
        </div>
      </div>
      <div class='book-nav-list'>
        <button
          v-for='tab in readerTabs'
          :key='tab.key'
          :class="['book-nav-item', activeManagerView === tab.key ? 'active' : '']"
          :title='sidebarCollapsed ? tab.label : undefined'
          @click='openManagerView(tab.key)'
        >
          <component :is='readerTabIcons[tab.key]' :size='17' :stroke-width='1.8' />
          <span>{{ tab.label }}</span>
        </button>
      </div>

      <template v-if='!groupDetail'>
        <div class='book-sidebar-divider'></div>
        <LibraryScanPanel
          v-model:selected-ids='selectedScanUserIds'
          :drive-options='scanAccountOptions'
          :is-scanning='bookStore.isScanning'
          :scanning-status-text="bookStore.scanLabel || '扫描中...'"
          :idle-status-text="bookStore.lastScanAt ? formatTime(bookStore.lastScanAt) : '尚未扫描'"
          import-label='导入本地图书'
          clear-confirm-text='确定清空整个书籍库？此操作不可恢复'
          :clear-disabled='!hasAnyBookRecords'
          @start-scan='scanBooks'
          @stop-scan='stopScan'
          @import-local='triggerLocalImport'
          @clear-library='clearLibrary'
        />
      </template>
    </aside>

    <main class='book-main'>
      <header class='book-header'>
        <div class='book-header-title-row'>
          <div class='book-header-title'>
            <h2>{{ activeManagerTitle }}</h2>
            <p>{{ activeManagerSubtitle }}</p>
          </div>
        </div>
        <div class='book-header-ctrl-row'>
          <span v-if='isCollectionManagerView' class='book-total-count'>合计 {{ readerVisibleBooks.length }} 本</span>
          <a-input v-model='query' allow-clear size='small' class='book-search' :placeholder='bookSearchPlaceholder'>
            <template #prefix><Search :size='15' :stroke-width='1.8' /></template>
          </a-input>
          <select v-if='isCollectionManagerView && !groupDetail' v-model='readingStatusFilter' class='book-status-filter' title='阅读状态'>
            <option value=''>全部状态</option>
            <option value='unread'>未读</option>
            <option value='reading'>阅读中</option>
            <option value='finished'>已读完</option>
          </select>
          <div class='book-header-divider' />
          <a-dropdown trigger='click'>
            <a-button size='small' class='book-sort-btn'>
              {{ sortModeLabel(bookStore.sortMode) }} · {{ bookStore.sortOrder === 'asc' ? '升序' : '降序' }}
            </a-button>
            <template #content>
              <div class='book-sort-dropdown'>
                <div class='book-sort-section-title'>排序方式</div>
                <div
                  v-for='opt in sortModeOptions'
                  :key='opt.value'
                  :class="['book-sort-item', bookStore.sortMode === opt.value ? 'active' : '']"
                  @click='handleSortModeChange(opt.value)'
                >
                  {{ opt.label }}
                </div>
                <div class='book-sort-divider' />
                <div class='book-sort-section-title'>排列顺序</div>
                <div
                  :class="['book-sort-item', bookStore.sortOrder === 'asc' ? 'active' : '']"
                  @click="bookStore.setSortOrder('asc')"
                >升序</div>
                <div
                  :class="['book-sort-item', bookStore.sortOrder === 'desc' ? 'active' : '']"
                  @click="bookStore.setSortOrder('desc')"
                >降序</div>
              </div>
            </template>
          </a-dropdown>
          <a-radio-group
            :model-value='bookStore.viewMode'
            type='button'
            size='small'
            class='book-view-toggle'
            @change='handleViewModeChange'
          >
            <a-radio value='grid' title='网格'>
              <Grid3X3 :size='14' :stroke-width='1.9' />
            </a-radio>
            <a-radio value='list' title='列表'>
              <List :size='14' :stroke-width='1.9' />
            </a-radio>
            <a-radio value='cover' title='封面'>
              <BookOpen :size='14' :stroke-width='1.9' />
            </a-radio>
          </a-radio-group>
          <div v-if='isCollectionManagerView && bookStore.viewMode === "grid"' class='book-card-scale'>
            <a-slider v-model='cardScale' :min='0.6' :max='2' :step='0.1' class='book-card-scale-slider' title='卡片缩放' />
          </div>
          <input
            ref='localFileInput'
            type='file'
            accept='.epub,.pdf,.mobi,.azw3,.txt,.azw,.djvu,.fb2,.cbr,.cbz,.cbt,.cb7,.docx,.md,.markdown,.html,.htm'
            multiple
            style='display:none'
            @change='handleLocalFileImport'
          />
          <a-button size='small' class='book-settings-btn' title='设置' @click='showManagerSettings = true'>
            <template #icon><Settings :size='15' :stroke-width='1.9' /></template>
          </a-button>
        </div>
      </header>

      <section class='book-content'>
        <a-empty v-if='!hasAnyBookRecords' description='暂无书籍，点击右上角“扫描”开始收录'>
          <template #image>
            <BookOpen :size='58' :stroke-width='1.5' style='color: var(--color-text-3)' />
          </template>
        </a-empty>

        <div v-if='selectedBookIds.length' class='book-batch-bar'>
          <span>已选 {{ selectedBookIds.length }} 本</span>
          <a-button size='mini' @click='toggleSelectAllVisibleBooks'>
            {{ isAllSelectableBooksSelected ? '取消全选' : '全选当前' }}
          </a-button>
          <template v-if='isTrashManagerView'>
            <a-button size='mini' @click='restoreSelectedBooks'>恢复</a-button>
            <a-button size='mini' status='danger' @click='permanentlyDeleteSelectedBooks'>永久删除</a-button>
          </template>
          <template v-else>
            <a-button size='mini' @click='assignSelectedBooksToShelf'>加入书架</a-button>
            <a-button size='mini' @click='setSelectedBooksFavorite(true)'>收藏</a-button>
            <a-button size='mini' @click='setSelectedBooksFavorite(false)'>取消收藏</a-button>
            <a-button size='mini' status='danger' @click='deleteSelectedBooks'>移入已删除</a-button>
          </template>
          <button class='book-batch-close' title='取消选择' @click='clearSelectedBooks'>
            <X :size='14' :stroke-width='2' />
          </button>
        </div>

        <template v-if='bookStore.totalCount && groupDetail'>
          <div class='book-group-header'>
            <a-button size='mini' @click='groupDetail = null'>返回</a-button>
            <span>{{ detailBooks.length }} 本</span>
            <span v-if="groupDetail.type === 'folder' && detailBooks[0]" class='book-source-pill'>{{ sourceLabel(detailBooks[0]) }}</span>
          </div>
          <a-empty v-if='!detailBooks.length' description='没有匹配的书籍' />
          <div v-else class='book-list book-list-linear'>
            <div
              v-for='book in detailBooks'
              :key='book.id'
              :class="['book-list-item', isBookSelected(book) ? 'multi-selected' : '']"
              role='button'
              tabindex='0'
              @click='openBook(book)'
              @keydown.enter='openBook(book)'
              @keydown.space.prevent='openBook(book)'
            >
              <button
                :class="['book-select-button', isBookSelected(book) ? 'checked' : '']"
                title='选择'
                @click='toggleBookSelected(book, $event)'
                @keydown.stop
              >
                <span v-if='isBookSelected(book)'>✓</span>
              </button>
              <div class='book-list-cover' :style="{ background: formatCoverColor(book) }">
                <img v-if='shouldUseCoverImage(book)' :src='bookCoverImage(book)' alt='' @error='onCoverImgError(book.id, $event)' />
                <span v-if='shouldShowCoverFallback(book)' class='book-list-cover-format'>{{ (book.ext || 'BOOK').toUpperCase() }}</span>
              </div>
              <div class='book-list-meta'>
                <div class='book-list-title'>{{ bookDisplayTitle(book) }}</div>
                <div class='book-list-sub'>
                  {{ book.author || '未知作者' }} · {{ (book.ext || '').toUpperCase() }} · {{ humanSize(book.size || 0) }}
                  <span v-if='readerBadgeFor(book)' :class="['book-engine-chip', 'book-engine-' + readerBadgeFor(book)!.tone]">
                    {{ readerBadgeFor(book)!.label }}
                  </span>
                  <span v-if='readingProgressLabel(book)' class='book-progress-chip'>{{ readingProgressLabel(book) }}</span>
                </div>
              </div>
              <button
                :class="['book-favorite-button', book.is_favorite ? 'active' : '']"
                title='收藏'
                @click='toggleBookFavorite(book, $event)'
                @keydown.stop
              >
                <Heart :size='16' :stroke-width='1.9' :fill="book.is_favorite ? 'currentColor' : 'none'" />
              </button>
            </div>
          </div>
        </template>

        <template v-else-if="activeManagerView === 'formats'">
          <div class='book-groups'>
            <button v-for='g in bookStore.byFormat' :key='g.format' class='book-group' @click='openGroupDetail("format", g.format, g.items)'>
              <FileText :size='20' :stroke-width='1.6' />
              <span>{{ g.format }}</span>
              <b>{{ g.count }}</b>
            </button>
          </div>
        </template>

        <template v-else-if="activeManagerView === 'shelves'">
          <div class='book-groups'>
            <button v-for='shelf in bookStore.byShelf' :key='shelf.id' class='book-group wide book-shelf-group' @click='openShelf(shelf)'>
              <LibraryBig :size='20' :stroke-width='1.6' />
              <span>
                <em>{{ shelf.name }}</em>
                <small>{{ shelf.id === DEFAULT_SHELF_ID ? '未加入自定义书架' : '自定义书架' }}</small>
              </span>
              <b v-if='managerPreferences.isShowShelfBookCount'>{{ shelf.book_ids.length }}</b>
            </button>
          </div>
        </template>

        <template v-else-if="activeManagerView === 'folders'">
          <div class='book-groups'>
            <div
              v-for='g in bookStore.byFolder'
              :key='g.key'
              class='book-group wide book-folder-group'
              :class="{ selected: selectedFolderKeys.includes(g.key) }"
              role='button'
              tabindex='0'
              @click='openFolderCard(g)'
              @keydown.enter='openFolderCard(g)'
              @keydown.space.prevent='openFolderCard(g)'
              @contextmenu.prevent='openFolderContextMenu($event, g)'
            >
              <button class='book-folder-check' :class="{ checked: selectedFolderKeys.includes(g.key) }" title='选择文件夹' @click.stop='toggleFolderSelected(g)' @keydown.stop>
                <span v-if='selectedFolderKeys.includes(g.key)'>✓</span>
              </button>
              <Folder :size='20' :stroke-width='1.6' />
              <span>
                <em>{{ g.path }}</em>
                <small>{{ folderSourceLabel(g) }}</small>
              </span>
              <b>{{ g.count }}</b>
            </div>
          </div>
        </template>

        <template v-else-if="activeManagerView === 'trash'">
          <div v-if='bookStore.deletedCount && !selectedBookIds.length' class='book-trash-toolbar'>
            <span>{{ bookStore.deletedCount }} 本已删除</span>
            <a-button size='mini' status='danger' @click='clearDeletedBooks'>清空已删除</a-button>
          </div>
          <a-empty v-if='!trashVisibleBooks.length' description='暂无已删除书籍'>
            <template #image>
              <Trash2 :size='54' :stroke-width='1.5' style='color: var(--color-text-3)' />
            </template>
          </a-empty>
          <div v-else class='book-list book-list-linear'>
            <div
              v-for='book in trashVisibleBooks'
              :key='book.id'
              :class="['book-list-item', 'trash-item', isBookSelected(book) ? 'multi-selected' : '']"
              role='button'
              tabindex='0'
              @click='toggleBookSelected(book, $event)'
              @keydown.enter='toggleBookSelected(book, $event)'
              @keydown.space.prevent='toggleBookSelected(book, $event)'
            >
              <button
                :class="['book-select-button', isBookSelected(book) ? 'checked' : '']"
                title='选择'
                @click='toggleBookSelected(book, $event)'
                @keydown.stop
              >
                <span v-if='isBookSelected(book)'>✓</span>
              </button>
              <div class='book-list-cover' :style="{ background: formatCoverColor(book) }">
                <img v-if='shouldUseCoverImage(book)' :src='bookCoverImage(book)' alt='' @error='onCoverImgError(book.id, $event)' />
                <span v-if='shouldShowCoverFallback(book)' class='book-list-cover-format'>{{ (book.ext || 'BOOK').toUpperCase() }}</span>
              </div>
              <div class='book-list-meta'>
                <div class='book-list-title'>{{ bookDisplayTitle(book) }}</div>
                <div class='book-list-sub'>
                  {{ book.author || '未知作者' }} · {{ (book.ext || '').toUpperCase() }} · {{ humanSize(book.size || 0) }}
                </div>
              </div>
              <div class='book-trash-actions'>
                <a-button size='mini' @click='restoreBook(book, $event)'>恢复</a-button>
                <a-button size='mini' status='danger' @click='permanentlyDeleteBook(book, $event)'>永久删除</a-button>
              </div>
            </div>
          </div>
        </template>

        <template v-else-if="activeManagerView === 'notes'">
          <a-empty v-if='!bookStore.allNotes.length' description='暂无笔记'>
            <template #image>
              <StickyNote :size='54' :stroke-width='1.5' style='color: var(--color-text-3)' />
            </template>
          </a-empty>
          <div v-else class='book-annotation-toolbar'>
            <div class='book-annotation-tags'>
              <span>标签</span>
              <button
                v-for='tag in annotationTagOptions'
                :key='tag'
                :class="['book-annotation-tag', selectedAnnotationTags.includes(tag) ? 'active' : '']"
                @click='toggleAnnotationTag(tag)'
              >
                {{ tag }}
              </button>
              <em v-if='!annotationTagOptions.length'>无标签</em>
            </div>
            <select v-model='selectedAnnotationBookId' class='book-annotation-select'>
              <option value=''>全部书籍</option>
              <option v-for='book in annotationBookOptions' :key='book.value' :value='book.value'>{{ book.label }}</option>
            </select>
            <select :value='annotationSortMode' class='book-annotation-select' @change='handleAnnotationSortModeChange'>
              <option v-for='opt in annotationSortModeOptions' :key='opt.value' :value='opt.value'>{{ opt.label }}</option>
            </select>
            <a-button size='mini' @click='toggleAnnotationSortOrder'>{{ annotationSortOrder === 'asc' ? '升序' : '降序' }}</a-button>
            <a-button v-if='selectedAnnotationTags.length || selectedAnnotationBookId' size='mini' @click='clearAnnotationFilters'>清除</a-button>
            <a-button size='mini' class='btn-tag-manage' :type="tagEditMode ? 'primary' : 'outline'" @click='toggleTagEditor'>
              <Tag :size='14' :stroke-width='1.8' /> {{ tagEditMode ? '收起标签管理' : '管理标签' }}
            </a-button>
            <select v-model='exportAllFormat' class='book-annotation-select' title='导出格式'>
              <option value='md'>Markdown</option>
              <option value='txt'>纯文本</option>
              <option value='html'>HTML</option>
              <option value='csv'>CSV</option>
            </select>
            <a-button size='mini' @click='handleExportAllAnnotations'>导出全部书摘</a-button>
          </div>
          <div v-if='tagEditMode' class='book-tag-editor'>
            <div class='book-tag-editor-list'>
              <div v-for='tag in bookStore.noteTags' :key='tag' class='book-tag-editor-item'>
                <template v-if='editingTag === tag'>
                  <input
                    v-model='editingTagValue'
                    class='book-tag-editor-input'
                    @keydown.enter='handleRenameTag()'
                    @keydown.escape='cancelRenameTag()'
                    @blur='cancelRenameTag()'
                  />
                </template>
                <span v-else class='book-tag-editor-name'>{{ tag }}</span>
                <button v-if='editingTag !== tag' class='btn-tag-rename' title='重命名' @click='startRenameTag(tag)'>
                  <PencilLine :size='13' :stroke-width='1.8' />
                </button>
                <button v-if='editingTag !== tag' class='btn-tag-delete' title='删除' @click='handleDeleteTag(tag)'>
                  <X :size='14' :stroke-width='2' />
                </button>
              </div>
              <em v-if='!bookStore.noteTags.length'>暂无全局标签</em>
            </div>
            <div class='book-tag-editor-add'>
              <input
                v-model='newTagName'
                placeholder='新建标签...'
                class='book-tag-editor-input'
                @keydown.enter='handleAddTag()'
              />
              <a-button size='mini' @click='handleAddTag' :disabled='!newTagName.trim()'>
                <Plus :size='14' :stroke-width='2' /> 添加
              </a-button>
            </div>
          </div>
          <a-empty v-if='bookStore.allNotes.length && !filteredNotes.length' description='没有匹配的笔记' />
          <div v-if='bookStore.allNotes.length && filteredNotes.length' class='book-annotation-list'>
            <button
              v-for='note in filteredNotes'
              :key='note.id'
              :class="['book-annotation-item', !bookForAnnotation(note) ? 'disabled' : '']"
              :disabled='!bookForAnnotation(note)'
              @click='openAnnotationBook(note, "note")'
            >
              <StickyNote :size='18' :stroke-width='1.7' />
              <span>
                <b>来自《{{ annotationBookTitle(note) }}》{{ note.chapter || '未知章节' }}</b>
                <em>{{ note.text || '无摘录内容' }}</em>
                <small v-if='note.note'>{{ note.note }}</small>
                <small v-if='note.tags?.length' class='book-annotation-item-tags'>
                  <i v-for='tag in note.tags' :key='tag'>{{ tag }}</i>
                </small>
              </span>
              <span class='book-annotation-actions'>
                <span role='button' tabindex='0' title='定位到书内' @click='showAnnotationInBook(note, "note", $event)' @keydown.enter='showAnnotationInBook(note, "note", $event)' @keydown.space.prevent='showAnnotationInBook(note, "note", $event)'>
                  <BookOpen :size='15' :stroke-width='1.8' />
                </span>
                <span role='button' tabindex='0' title='编辑笔记' @click='editAnnotationInBook(note, "note", $event)' @keydown.enter='editAnnotationInBook(note, "note", $event)' @keydown.space.prevent='editAnnotationInBook(note, "note", $event)'>
                  <Edit3 :size='15' :stroke-width='1.8' />
                </span>
                <span role='button' tabindex='0' title='复制笔记链接' @click='copyAnnotationLink(note, $event)' @keydown.enter='copyAnnotationLink(note, $event)' @keydown.space.prevent='copyAnnotationLink(note, $event)'>
                  <Copy :size='15' :stroke-width='1.8' />
                </span>
                <span role='button' tabindex='0' title='删除笔记' @click='deleteAnnotation(note, $event)' @keydown.enter='deleteAnnotation(note, $event)' @keydown.space.prevent='deleteAnnotation(note, $event)'>
                  <Trash2 :size='15' :stroke-width='1.8' />
                </span>
              </span>
            </button>
          </div>
        </template>

        <template v-else-if="activeManagerView === 'highlights'">
          <a-empty v-if='!bookStore.allHighlights.length' description='暂无书摘'>
            <template #image>
              <Highlighter :size='54' :stroke-width='1.5' style='color: var(--color-text-3)' />
            </template>
          </a-empty>
          <div v-else class='book-annotation-toolbar'>
            <div class='book-annotation-tags'>
              <span>标签</span>
              <button
                v-for='tag in annotationTagOptions'
                :key='tag'
                :class="['book-annotation-tag', selectedAnnotationTags.includes(tag) ? 'active' : '']"
                @click='toggleAnnotationTag(tag)'
              >
                {{ tag }}
              </button>
              <em v-if='!annotationTagOptions.length'>无标签</em>
            </div>
            <select v-model='selectedAnnotationBookId' class='book-annotation-select'>
              <option value=''>全部书籍</option>
              <option v-for='book in annotationBookOptions' :key='book.value' :value='book.value'>{{ book.label }}</option>
            </select>
            <select :value='annotationSortMode' class='book-annotation-select' @change='handleAnnotationSortModeChange'>
              <option v-for='opt in annotationSortModeOptions' :key='opt.value' :value='opt.value'>{{ opt.label }}</option>
            </select>
            <a-button size='mini' @click='toggleAnnotationSortOrder'>{{ annotationSortOrder === 'asc' ? '升序' : '降序' }}</a-button>
            <a-button v-if='selectedAnnotationTags.length || selectedAnnotationBookId' size='mini' @click='clearAnnotationFilters'>清除</a-button>
            <a-button size='mini' class='btn-tag-manage' :type="tagEditMode ? 'primary' : 'outline'" @click='toggleTagEditor'>
              <Tag :size='14' :stroke-width='1.8' /> {{ tagEditMode ? '收起标签管理' : '管理标签' }}
            </a-button>
            <select v-model='exportAllFormat' class='book-annotation-select' title='导出格式'>
              <option value='md'>Markdown</option>
              <option value='txt'>纯文本</option>
              <option value='html'>HTML</option>
              <option value='csv'>CSV</option>
            </select>
            <a-button size='mini' @click='handleExportAllAnnotations'>导出全部书摘</a-button>
          </div>
          <div v-if='tagEditMode' class='book-tag-editor'>
            <div class='book-tag-editor-list'>
              <div v-for='tag in bookStore.noteTags' :key='tag' class='book-tag-editor-item'>
                <template v-if='editingTag === tag'>
                  <input
                    v-model='editingTagValue'
                    class='book-tag-editor-input'
                    @keydown.enter='handleRenameTag()'
                    @keydown.escape='cancelRenameTag()'
                    @blur='cancelRenameTag()'
                  />
                </template>
                <span v-else class='book-tag-editor-name'>{{ tag }}</span>
                <button v-if='editingTag !== tag' class='btn-tag-rename' title='重命名' @click='startRenameTag(tag)'>
                  <PencilLine :size='13' :stroke-width='1.8' />
                </button>
                <button v-if='editingTag !== tag' class='btn-tag-delete' title='删除' @click='handleDeleteTag(tag)'>
                  <X :size='14' :stroke-width='2' />
                </button>
              </div>
              <em v-if='!bookStore.noteTags.length'>暂无全局标签</em>
            </div>
            <div class='book-tag-editor-add'>
              <input
                v-model='newTagName'
                placeholder='新建标签...'
                class='book-tag-editor-input'
                @keydown.enter='handleAddTag()'
              />
              <a-button size='mini' @click='handleAddTag' :disabled='!newTagName.trim()'>
                <Plus :size='14' :stroke-width='2' /> 添加
              </a-button>
            </div>
          </div>
          <a-empty v-if='bookStore.allHighlights.length && !filteredHighlights.length' description='没有匹配的书摘' />
          <div v-if='bookStore.allHighlights.length && filteredHighlights.length' class='book-annotation-list'>
            <button
              v-for='highlight in filteredHighlights'
              :key='highlight.id'
              :class="['book-annotation-item', !bookForAnnotation(highlight) ? 'disabled' : '']"
              :disabled='!bookForAnnotation(highlight)'
              @click='openAnnotationBook(highlight, "highlight")'
            >
              <Highlighter :size='18' :stroke-width='1.7' />
              <span>
                <b>来自《{{ annotationBookTitle(highlight) }}》{{ highlight.chapter || '未知章节' }}</b>
                <em>{{ highlight.text || '无摘录内容' }}</em>
                <small v-if='highlight.tags?.length' class='book-annotation-item-tags'>
                  <i v-for='tag in highlight.tags' :key='tag'>{{ tag }}</i>
                </small>
              </span>
              <span class='book-annotation-actions'>
                <span role='button' tabindex='0' title='定位到书内' @click='showAnnotationInBook(highlight, "highlight", $event)' @keydown.enter='showAnnotationInBook(highlight, "highlight", $event)' @keydown.space.prevent='showAnnotationInBook(highlight, "highlight", $event)'>
                  <BookOpen :size='15' :stroke-width='1.8' />
                </span>
                <span role='button' tabindex='0' title='编辑书摘' @click='editAnnotationInBook(highlight, "highlight", $event)' @keydown.enter='editAnnotationInBook(highlight, "highlight", $event)' @keydown.space.prevent='editAnnotationInBook(highlight, "highlight", $event)'>
                  <Edit3 :size='15' :stroke-width='1.8' />
                </span>
                <span role='button' tabindex='0' title='复制笔记链接' @click='copyAnnotationLink(highlight, $event)' @keydown.enter='copyAnnotationLink(highlight, $event)' @keydown.space.prevent='copyAnnotationLink(highlight, $event)'>
                  <Copy :size='15' :stroke-width='1.8' />
                </span>
                <span role='button' tabindex='0' title='删除书摘' @click='deleteAnnotation(highlight, $event)' @keydown.enter='deleteAnnotation(highlight, $event)' @keydown.space.prevent='deleteAnnotation(highlight, $event)'>
                  <Trash2 :size='15' :stroke-width='1.8' />
                </span>
              </span>
            </button>
          </div>
        </template>

        <template v-else-if="activeManagerView === 'bookmarks'">
          <a-empty v-if='!bookStore.allBookmarks.length' description='暂无书签'>
            <template #image>
              <Bookmark :size='54' :stroke-width='1.5' style='color: var(--color-text-3)' />
            </template>
          </a-empty>
          <a-empty v-else-if='!filteredBookmarks.length' description='没有匹配的书签' />
          <div v-else class='book-annotation-list'>
            <button
              v-for='bookmark in filteredBookmarks'
              :key='bookmark.id'
              :class="['book-annotation-item', !bookForAnnotation(bookmark) ? 'disabled' : '']"
              :disabled='!bookForAnnotation(bookmark)'
              @click='openAnnotationBook(bookmark, "bookmark")'
            >
              <Bookmark :size='18' :stroke-width='1.7' />
              <span>
                <b>来自《{{ annotationBookTitle(bookmark) }}》{{ bookmark.chapter || '未知章节' }}</b>
                <em>{{ bookmark.label || '书签' }}</em>
                <small>{{ bookmarkPercentLabel(bookmark) }}</small>
              </span>
              <span class='book-annotation-actions'>
                <span role='button' tabindex='0' title='定位到书内' @click='showAnnotationInBook(bookmark, "bookmark", $event)' @keydown.enter='showAnnotationInBook(bookmark, "bookmark", $event)' @keydown.space.prevent='showAnnotationInBook(bookmark, "bookmark", $event)'>
                  <BookOpen :size='15' :stroke-width='1.8' />
                </span>
              </span>
            </button>
          </div>
        </template>

        <template v-else-if="activeManagerView === 'stats'">
          <StatsPage @close="activeManagerView = 'home'" />
        </template>

        <template v-else-if='isCollectionManagerView'>
          <a-empty v-if='!readerVisibleBooks.length' description='没有匹配的书籍' />
          <template v-else-if="bookStore.viewMode === 'list'">
            <div class='book-list book-list-linear'>
              <div
                v-for='book in readerVisibleBooks'
                :key='book.id'
                :class="['book-list-item', isBookSelected(book) ? 'multi-selected' : '']"
                role='button'
                tabindex='0'
                @click='openBook(book)'
                @keydown.enter='openBook(book)'
                @keydown.space.prevent='openBook(book)'
                @contextmenu.prevent='openBookContextMenu($event, book)'
              >
                <button
                  :class="['book-select-button', isBookSelected(book) ? 'checked' : '']"
                  title='选择'
                  @click='toggleBookSelected(book, $event)'
                  @keydown.stop
                >
                  <span v-if='isBookSelected(book)'>✓</span>
                </button>
                <div class='book-list-cover' :style="{ background: formatCoverColor(book) }">
                  <img v-if='shouldUseCoverImage(book)' :src='bookCoverImage(book)' alt='' @error='onCoverImgError(book.id, $event)' />
                  <span v-if='shouldShowCoverFallback(book)' class='book-list-cover-format'>{{ (book.ext || 'BOOK').toUpperCase() }}</span>
                </div>
                <div class='book-list-meta'>
                  <div class='book-list-title'>{{ bookDisplayTitle(book) }}</div>
                  <div class='book-list-sub'>
                    {{ book.author || '未知作者' }} · {{ (book.ext || '').toUpperCase() }} · {{ humanSize(book.size || 0) }}
                    <span v-if='readerBadgeFor(book)' :class="['book-engine-chip', 'book-engine-' + readerBadgeFor(book)!.tone]">
                      {{ readerBadgeFor(book)!.label }}
                    </span>
                    <span v-if='readingProgressLabel(book)' class='book-progress-chip'>{{ readingProgressLabel(book) }}</span>
                  </div>
                </div>
                <button
                  :class="['book-favorite-button', book.is_favorite ? 'active' : '']"
                  title='收藏'
                  @click='toggleBookFavorite(book, $event)'
                  @keydown.stop
                >
                  <Heart :size='16' :stroke-width='1.9' :fill="book.is_favorite ? 'currentColor' : 'none'" />
                </button>
              </div>
            </div>
          </template>
          <template v-else-if="bookStore.viewMode === 'cover'">
            <div class='book-cover-list'>
              <div
                v-for='book in readerVisibleBooks'
                :key='book.id'
                :class="['book-cover-item', isBookSelected(book) ? 'multi-selected' : '']"
                @click='openBook(book)'
                @contextmenu.prevent='openBookContextMenu($event, book)'
              >
                <div class='book-cover-item-header'>
                  <span v-if='readingProgressLabel(book)' class='book-cover-item-progress'>{{ readingProgressLabel(book) }}</span>
                  <span v-if='book.is_favorite' class='book-cover-item-favorite'>
                    <Heart :size='14' :stroke-width='2' :fill="book.is_favorite ? 'currentColor' : 'none'" />
                  </span>
                </div>
                <button
                  :class="['book-select-button book-cover-select', isBookSelected(book) ? 'checked' : '']"
                  title='选择'
                  @click.stop='toggleBookSelected(book, $event)'
                >
                  <span v-if='isBookSelected(book)'>✓</span>
                </button>
                <div class='book-cover-item-cover' :style="{ background: formatCoverColor(book) }">
                  <img v-if='shouldUseCoverImage(book)' :src='bookCoverImage(book)' alt='' @error='onCoverImgError(book.id, $event)' />
                  <span v-if='shouldShowCoverFallback(book)' class='book-cover-item-cover-format'>{{ (book.ext || 'BOOK').toUpperCase() }}</span>
                </div>
                <div class='book-cover-item-body'>
                  <div class='book-cover-item-title'>{{ bookDisplayTitle(book) }}</div>
                  <div class='book-cover-item-author'>作者：{{ book.author || '未知作者' }}</div>
                  <div v-if='book.publisher' class='book-cover-item-publisher'>出版社：{{ book.publisher }}</div>
                  <div class='book-cover-item-desc'>
                    <div class='book-cover-item-desc-text'>{{ book.summary || book.description || '暂无简介' }}</div>
                  </div>
                  <div class='book-cover-item-meta'>
                    <span>{{ (book.ext || '').toUpperCase() }}</span>
                    <span>{{ humanSize(book.size || 0) }}</span>
                  </div>
                </div>
              </div>
            </div>
          </template>
          <template v-else>
            <div class='book-card-grid' :style="{ '--card-scale': cardScale }">
              <div
                v-for='book in readerVisibleBooks'
                :key='book.id'
                :class="['book-card-item', isBookSelected(book) ? 'multi-selected' : '']"
                :style="{ '--cover-color': formatCoverColor(book) }"
                @click='openBook(book)'
                @contextmenu.prevent='openBookContextMenu($event, book)'
              >
                <div class='book-card-item-cover'>
                  <img v-if='shouldUseCoverImage(book)' :src='bookCoverImage(book)' alt='' @error='onCoverImgError(book.id, $event)' />
                  <span v-if='shouldShowCoverFallback(book)' class='book-card-item-format'>{{ (book.ext || 'BOOK').toUpperCase() }}</span>
                  <button
                    :class="['book-select-button book-card-select', isBookSelected(book) ? 'checked' : '']"
                    title='选择'
                    @click.stop='toggleBookSelected(book, $event)'
                  >
                    <span v-if='isBookSelected(book)'>✓</span>
                  </button>
                  <span v-if='book.is_favorite' class='book-card-item-favorite' @click.stop='toggleBookFavorite(book, $event)'>
                    <Heart :size='12' :stroke-width='2' fill='currentColor' />
                  </span>
                </div>
                <div class='book-card-item-title'>{{ bookDisplayTitle(book) }}</div>
                <span v-if='readingProgressLabel(book)' class='book-card-item-progress'>{{ readingProgressLabel(book) }}</span>
              </div>
            </div>
          </template>
        </template>
      </section>
    </main>

    <aside :class="['book-detail', selectedBook ? 'visible' : '']">
      <template v-if='selectedBook'>
        <button class='book-detail-close' title='放回书架' @click='putBackBook'>
          <X :size='17' :stroke-width='2' />
        </button>
        <div class='book-detail-cover' :style="{ background: formatCoverColor(selectedBook) }">
          <img v-if='shouldUseCoverImage(selectedBook)' :src='bookCoverImage(selectedBook)' alt='' @error='onCoverImgError(selectedBook.id, $event)' />
          <div v-if='shouldShowCoverFallback(selectedBook)' class='book-detail-cover-empty'>
            <span class='book-detail-cover-format'>{{ (selectedBook.ext || 'BOOK').toUpperCase() }}</span>
            <span class='book-detail-cover-title'>{{ emptyCoverTitle(selectedBook) }}</span>
          </div>
        </div>
        <h3>{{ bookDisplayTitle(selectedBook) }}</h3>
        <div class='book-detail-author'>{{ selectedBook.author || '未知作者' }}</div>
        <p>{{ selectedBook.summary || selectedBook.description || '暂无简介' }}</p>
        <div class='book-detail-grid'>
          <span>ISBN</span><b>{{ selectedBook.isbn || '未识别' }}</b>
          <span>出版社</span><b>{{ selectedBook.publisher || '未知' }}</b>
          <span>出版</span><b>{{ selectedBook.published_date || '未知' }}</b>
          <span>格式</span><b>{{ (selectedBook.ext || '').toUpperCase() }}</b>
          <span>大小</span><b>{{ humanSize(selectedBook.size || 0) }}</b>
          <span>账号</span><b>{{ sourceLabel(selectedBook) }}</b>
          <span>来源</span><b>{{ selectedBook.parent_path || '根目录' }}</b>
          <span>书架</span><b>{{ shelfNameForBook(selectedBook) }}</b>
        </div>
        <div class='book-detail-actions'>
          <a-button size='small' long @click='assignSelectedBookToShelf'>加入书架</a-button>
          <a-button v-if='selectedBook.shelf_id' size='small' long type='text' @click='removeSelectedBookFromShelf'>移回默认书架</a-button>
        </div>
        <div v-if='selectedBook.subjects && selectedBook.subjects.length' class='book-subjects'>
          <span v-for='s in selectedBook.subjects' :key='s'>{{ s }}</span>
        </div>
      </template>
    </aside>
    <a-modal
      v-model:visible='showManagerSettings'
      :footer='false'
      :closable='false'
      :mask-closable='true'
      modal-class='manager-settings-modal'
      width='min(96vw, 1120px)'
    >
      <div class='manager-settings-shell'>
        <aside class='manager-settings-sidebar'>
          <h2>设置</h2>
          <button
            v-for='tab in BOOK_MANAGER_SETTING_TABS'
            :key='tab.key'
            :class="['manager-settings-tab', activeSettingTab === tab.key ? 'active' : '']"
            @click='activeSettingTab = tab.key'
          >
            <component :is='settingTabIcons[tab.key]' :size='20' :stroke-width='1.85' />
            <span>{{ tab.label }}</span>
          </button>
          <hr class='manager-settings-divider' />
          <button
            v-for='tab in BOOK_MANAGER_SETTING_TABS_GROUP2'
            :key='tab.key'
            :class="['manager-settings-tab', activeSettingTab === tab.key ? 'active' : '']"
            @click='activeSettingTab = tab.key'
          >
            <component :is='settingTabIcons[tab.key]' :size='20' :stroke-width='1.85' />
            <span>{{ tab.label }}</span>
          </button>
        </aside>
        <section class='manager-settings-main'>
          <button class='manager-settings-close' title='关闭' @click='showManagerSettings = false'>
            <X :size='30' :stroke-width='1.7' />
          </button>
          <h2>{{ activeSettingTitle }}</h2>
          <div class='manager-settings-scroll'>
            <div v-if='settingSwitchGroups[activeSettingTab].length' class='manager-setting-group'>
              <div
                v-for='item in settingSwitchGroups[activeSettingTab]'
                :key='item.key'
                class='manager-setting-row'
              >
                <span>
                  <b>{{ item.label }}</b>
                  <em v-if='item.desc'>{{ item.desc }}</em>
                </span>
                <a-switch :model-value='Boolean(managerPreferences[item.key])' @change='toggleManagerPreference(item.key)' />
              </div>
            </div>

            <template v-if="activeSettingTab === 'general'">
              <div class='manager-setting-subsection'>
                <label class='manager-setting-field'>
                  <span>界面语言</span>
                  <select :value='readerLocale' @change='setReaderLocale(($event.target as HTMLSelectElement).value)'>
                    <option value='zh'>中文 (简体)</option>
                    <option value='zh-TW'>中文 (繁體)</option>
                    <option value='en'>English</option>
                  </select>
                </label>
                <label class='manager-setting-field'>
                  <span>默认搜索引擎</span>
                  <select :value='managerPreferences.defaultSearchEngine' @change='handleManagerSelectChange("defaultSearchEngine", $event)'>
                    <option v-for='engine in searchEngineOptions' :key='engine.value' :value='engine.value'>{{ engine.label }}</option>
                  </select>
                </label>
                <label class='manager-setting-field'>
                  <span>启动时打开书架</span>
                  <input :value='managerPreferences.startupShelf' placeholder='留空则打开全部图书' @input='setManagerPreferenceText("startupShelf", ($event.target as HTMLInputElement).value)' />
                </label>
              </div>
            </template>

            <template v-if="activeSettingTab === 'data'">
              <div class='manager-setting-subsection'>
                <div class='manager-setting-note'>账号登录和数据同步已按当前产品边界排除；书籍库数据继续使用现有本地数据库和网盘扫描结果。</div>
                <div class='manager-setting-actions'>
                  <a-button size='mini' @click='handleExportAllAnnotations'>导出全部书摘</a-button>
                  <a-button size='mini' status='danger' :disabled='!hasAnyBookRecords' @click='clearLibrary'>清空资料库</a-button>
                </div>
              </div>
            </template>

            <template v-if="activeSettingTab === 'reading'">
              <div class='manager-setting-subsection'>
                <div class='manager-setting-actions'>
                  <a-button size='mini' @click='message.info("阅读器窗口位置将随全屏阅读页接入时复用该设置")'>重置阅读窗口位置</a-button>
                </div>
              </div>
            </template>

            <template v-if="activeSettingTab === 'appearance'">
              <div class='manager-setting-subsection'>
                <label class='manager-setting-field inline'>
                  <span>系统字体</span>
                  <select :value='managerPreferences.systemFont' @change='handleManagerSelectChange("systemFont", $event)'>
                    <option v-for='font in systemFontOptions' :key='font.value' :value='font.value'>{{ font.label }}</option>
                  </select>
                </label>
                <div class='manager-setting-title'>选择主题色</div>
                <div class='manager-theme-list'>
                  <button
                    v-for='theme in BOOK_MANAGER_THEME_COLORS'
                    :key='theme.value'
                    :class="['manager-theme-item', managerPreferences.themeColor === theme.value ? 'active' : '']"
                    :style="{ '--theme-color': theme.color }"
                    @click='setThemeColor(theme.value)'
                  >
                    <i></i>
                    <span>{{ theme.label }}</span>
                  </button>
                  <label :class="['manager-theme-item', /^#[0-9a-fA-F]{6}$/.test(managerPreferences.themeColor) && !BOOK_MANAGER_THEME_COLORS.some(theme => theme.value === managerPreferences.themeColor) ? 'active' : '']">
                    <i class='custom'></i>
                    <input v-model='customColorDraft' type='color' @change='setCustomThemeColor' />
                    <span>自定义</span>
                  </label>
                </div>
                <label v-if='managerPreferences.isCustomSystemCSS' class='manager-setting-field block'>
                  <span>自定义 CSS</span>
                  <textarea :value='managerPreferences.customSystemCSS' spellcheck='false' @input='setManagerPreferenceText("customSystemCSS", ($event.target as HTMLTextAreaElement).value)'></textarea>
                </label>
              </div>
            </template>

            <template v-if="activeSettingTab === 'more'">
              <div class='manager-setting-subsection'>
                <div class='manager-setting-row'>
                  <span>
                    <b>启用软件保护</b>
                    <em>启动应用时需要验证身份</em>
                  </span>
                  <a-switch v-model='softwareProtectionEnabled' size='small' />
                </div>
                <label v-if='softwareProtectionEnabled' class='manager-setting-field'>
                  <span>保护方式</span>
                  <select v-model='softwareProtectionMethod'>
                    <option value='password'>密码</option>
                    <option value='pin'>PIN 码</option>
                  </select>
                </label>
              </div>
            </template>

            <template v-if="activeSettingTab === 'plugins'">
              <div class='manager-setting-subsection'>
                <div class='manager-setting-note'>插件入口已保留。后续接入 Koodo 插件运行时后，会在这里显示已安装插件和启用状态。</div>
              </div>
            </template>

            <template v-if="activeSettingTab === 'ai'">
              <div class='manager-setting-subsection'>
                <div class='manager-setting-row'>
                  <span>
                    <b>禁用 AI 功能</b>
                    <em>仅影响书籍页入口偏好，已有全局 AI 配置仍保留</em>
                  </span>
                  <a-switch :model-value='managerPreferences.isDisableAI' @change='toggleManagerPreference("isDisableAI")' />
                </div>
                <div class='manager-setting-note'>AI 模型配置请在 主界面 → 设置 → API 密钥 中管理</div>
              </div>
            </template>

            <template v-if="activeSettingTab === 'txt'">
              <div class='manager-setting-subsection'>
                <div class='manager-resource-list'>
                  <div v-for='parser in managerPreferences.txtParsers' :key='parser.label' class='manager-resource-item'>
                    <span>
                      <b>{{ parser.label }}</b>
                      <em>{{ parser.regex }}</em>
                    </span>
                    <button class='danger' @click='deleteTxtParser(parser.label)'>删除</button>
                  </div>
                </div>
                <div class='manager-parser-add'>
                  <input v-model='txtParserDraft.label' placeholder='解析器名称' />
                  <input v-model='txtParserDraft.regex' placeholder='章节正则表达式' />
                  <a-button size='mini' @click='addTxtParser'>添加解析器</a-button>
                </div>
              </div>
            </template>

            <template v-if="activeSettingTab === 'dict'">
              <div class='manager-setting-subsection'>
                <div class='manager-resource-list'>
                  <div v-for='dict in managerPreferences.localDictionaries' :key='dict.id' class='manager-resource-item'>
                    <span>
                      <b>{{ dict.name }}</b>
                      <em>{{ dict.extension.toUpperCase() }}</em>
                    </span>
                    <button class='danger' @click='deleteLocalDictionary(dict.id)'>删除</button>
                  </div>
                  <div v-if='!managerPreferences.localDictionaries.length' class='manager-setting-note'>暂无本地词典。</div>
                </div>
                <div class='manager-parser-add'>
                  <input v-model='dictionaryDraftName' placeholder='MDX 词典名称' />
                  <a-button size='mini' @click='addLocalDictionary'>添加词典</a-button>
                </div>
              </div>
            </template>

            <template v-if="activeSettingTab === 'about'">
              <div class='manager-setting-subsection'>
                <div class='manager-about-card'>
                  <BookOpen :size='34' :stroke-width='1.6' />
                  <span>
                    <b>Koodo Reader for BoxPlayer</b>
                    <em>首页设置和阅读器迁移使用现有书籍页、网盘扫描与 AI 能力；账号和同步服务不接入。</em>
                  </span>
                </div>
                <div class='manager-setting-actions' style='margin-top:16px'>
                  <a-button size='mini' @click="message.info('已是最新版本')">检查更新</a-button>
                  <a-button size='mini' @click="openExternalLink('https://github.com/gaozhangmin/boxplayer')">GitHub</a-button>
                  <a-button size='mini' @click="openExternalLink('https://xbyvideohub.com/')">官方网站</a-button>
                  <a-button size='mini' @click="copyContactEmail">联系邮箱</a-button>
                </div>
              </div>
            </template>
          </div>
        </section>
      </div>
    </a-modal>
    <a-dropdown
      class='rightmenu'
      :popup-visible='folderContextVisible'
      :style='folderContextStyle'
      @popup-visible-change='closeFolderContextMenu'
    >
      <div style='width: 1px; height: 1px; visibility: hidden;' />
      <template #content>
        <div class='book-folder-menu'>
          <button class='book-folder-menu-item' @click='toggleContextFolderSelected'>
            {{ folderContextSelected ? '取消选择' : '选择文件夹' }}
          </button>
          <button class='book-folder-menu-item danger' @click='deleteCurrentFolderFromMenu'>
            移入已删除
          </button>
          <button class='book-folder-menu-item danger' :disabled='!selectedFolderGroups.length' @click='deleteSelectedFoldersFromMenu'>
            移入已删除（{{ selectedFolderGroups.length }}）
          </button>
        </div>
      </template>
    </a-dropdown>
    <a-dropdown
      class='rightmenu'
      :popup-visible='bookContextVisible'
      :style='bookContextStyle()'
      @popup-visible-change='(v: boolean) => { if (!v) closeBookContextMenu() }'
    >
      <div style='width: 1px; height: 1px; visibility: hidden;' />
      <template #content>
        <div class='book-folder-menu'>
          <button class='book-folder-menu-item' @click='bookContextToggleFavorite'>
            {{ bookContextBook?.is_favorite ? '取消收藏' : '加入收藏' }}
          </button>
          <button class='book-folder-menu-item' @click='bookContextMoveToShelf'>
            {{ bookContextBook?.shelf_id ? '更换书架' : '加入书架' }}
          </button>
          <button v-if='bookContextBook?.shelf_id' class='book-folder-menu-item' @click='bookContextRemoveFromShelf'>
            移回默认书架
          </button>
          <button class='book-folder-menu-item' @click='copyBookLink(bookContextBook!)'>
            复制书籍链接
          </button>
          <button class='book-folder-menu-item danger' @click='bookContextDelete'>
            移入已删除
          </button>
          <button class='book-folder-menu-item danger' @click='permanentlyDeleteBook(bookContextBook!)'>
            永久删除
          </button>
        </div>
      </template>
    </a-dropdown>
    <BookReaderModal v-model:visible='readerVisible' :book='selectedBook' :initial-annotation-target='pendingAnnotationTarget' />
  </a-layout>
</template>

<style scoped>
.library-settings-toggle {
  margin-top: 4px;
  text-align: center;
}
.library-settings-toggle a {
  font-size: 12px;
  color: var(--color-text-3);
  cursor: pointer;
}
.library-settings-toggle a:hover { color: rgb(var(--primary-6)); }
</style>
<style scoped>
.book-library {
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: row;
  background: transparent;
  color: var(--app-mineradio-ink, #e8ecef);
  font-family: var(--book-manager-font-family, inherit);
  position: relative;
}

.book-library::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  background-image: var(--book-manager-background-image);
  background-size: cover;
  background-position: center;
  opacity: .12;
  z-index: 0;
}

.book-library > * {
  position: relative;
  z-index: 1;
}

.book-library.cover-no-crop .book-card-item-cover img,
.book-library.cover-no-crop .book-cover-item-cover img,
.book-library.cover-no-crop .book-list-cover img,
.book-library.cover-no-crop .book-detail-cover img {
  object-fit: contain;
  background: rgba(0, 0, 0, .05);
}

.book-library :deep(.arco-btn-primary),
.book-library :deep(.arco-switch-checked) {
  background-color: var(--book-manager-primary) !important;
}

.book-library :deep(.arco-slider-button),
.book-library :deep(.arco-slider-bar) {
  border-color: var(--book-manager-primary);
  background-color: var(--book-manager-primary);
}

:global(.manager-settings-modal .arco-modal),
:global(.arco-modal.manager-settings-modal) {
  width: 960px !important;
  max-width: calc(100vw - 24px);
  border-radius: 8px;
  overflow: hidden;
  padding: 0;
}

:global(.manager-settings-modal .arco-modal-body),
:global(.arco-modal.manager-settings-modal .arco-modal-body) {
  padding: 0;
}

:global(.manager-settings-modal .arco-modal-mask) {
  background: rgba(0, 0, 0, .42);
}

:global(.manager-settings-modal .arco-modal-content),
:global(.arco-modal.manager-settings-modal .arco-modal-content) {
  padding: 0;
  background: #fff;
}

:global(.manager-settings-shell) {
  height: min(76vh, 780px);
  min-height: 560px;
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr);
  color: #4a4a4a;
  background: linear-gradient(135deg, rgba(57, 147, 147, .05), transparent 42%), #fff;
}

:global(.manager-settings-sidebar) {
  padding: 26px 18px 20px 24px;
  border-right: 1px solid rgba(0, 0, 0, .08);
  overflow: auto;
}

:global(.manager-settings-sidebar h2),
:global(.manager-settings-main h2) {
  margin: 0 0 24px;
  font-size: 30px;
  line-height: 1.15;
  font-weight: 800;
  letter-spacing: 0;
}

:global(.manager-settings-tab) {
  width: 100%;
  height: 52px;
  border: none;
  border-radius: 8px;
  padding: 0 14px;
  background: transparent;
  color: #969696;
  display: flex;
  align-items: center;
  gap: 14px;
  font-size: 19px;
  font-weight: 700;
  cursor: pointer;
}

:global(.manager-settings-tab.active) {
  background: var(--book-manager-primary, #399393);
  color: #fff;
}

:global(.manager-settings-divider) {
  margin: 8px 18px;
  border: none;
  border-top: 1px solid rgba(0, 0, 0, 0.08);
}

:global(.manager-settings-main) {
  position: relative;
  min-width: 0;
  padding: 28px 38px 30px;
  overflow: hidden;
}

:global(.manager-settings-close) {
  position: absolute;
  top: 24px;
  right: 28px;
  width: 42px;
  height: 42px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: #4d4d4d;
  display: grid;
  place-items: center;
  cursor: pointer;
}

:global(.manager-settings-close:hover) {
  background: rgba(0, 0, 0, .06);
}

:global(.manager-settings-scroll) {
  height: calc(100% - 58px);
  overflow: auto;
  padding: 0 6px 28px 0;
}

:global(.manager-setting-group) {
  display: grid;
  gap: 8px;
}

:global(.manager-setting-row) {
  min-height: 48px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 28px;
}

:global(.manager-setting-row span) {
  min-width: 0;
  display: grid;
  gap: 4px;
}

:global(.manager-setting-row b),
:global(.manager-setting-field > span),
:global(.manager-setting-title) {
  color: #4d4d4d;
  font-size: 20px;
  line-height: 1.2;
  font-weight: 800;
}

:global(.manager-setting-row em),
:global(.manager-setting-note),
:global(.manager-resource-item em),
:global(.manager-about-card em) {
  color: #818181;
  font-size: 14px;
  line-height: 1.45;
  font-style: normal;
}

:global(.manager-setting-row .arco-switch) {
  flex: 0 0 auto;
}

:global(.manager-setting-subsection) {
  margin-top: 22px;
  display: grid;
  gap: 18px;
}

:global(.manager-setting-field) {
  display: grid;
  gap: 8px;
}

:global(.manager-setting-field.inline) {
  max-width: 360px;
}

:global(.manager-setting-field input),
:global(.manager-setting-field select),
:global(.manager-setting-field textarea),
:global(.manager-parser-add input),
:global(.manager-background-add input) {
  min-height: 36px;
  border: 2px solid rgba(0, 0, 0, .18);
  border-radius: 6px;
  background: rgba(255, 255, 255, .88);
  color: var(--color-text-1, #222);
  font-size: 15px;
  padding: 0 10px;
}

:global(.manager-setting-field textarea) {
  min-height: 150px;
  resize: vertical;
  padding: 10px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

:global(.manager-theme-list) {
  display: grid;
  grid-template-columns: repeat(4, minmax(120px, 1fr));
  gap: 14px 26px;
  max-width: 850px;
}

:global(.manager-theme-item) {
  min-width: 0;
  height: 40px;
  border: 2px solid transparent;
  border-radius: 999px;
  background: transparent;
  color: var(--theme-color, #399393);
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 0 14px;
  font-size: 17px;
  font-weight: 800;
  cursor: pointer;
}

:global(.manager-theme-item.active) {
  border-color: var(--theme-color, #399393);
}

:global(.manager-theme-item i) {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: var(--theme-color, #399393);
}

:global(.manager-theme-item i.custom) {
  background: var(--book-manager-primary, #399393);
}

:global(.manager-theme-item input[type='color']) {
  width: 0;
  height: 0;
  opacity: 0;
  position: absolute;
}

:global(.manager-setting-actions),
:global(.manager-parser-add) {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

:global(.manager-resource-list) {
  display: grid;
  gap: 8px;
}

:global(.manager-resource-item) {
  min-height: 48px;
  border: 1px solid rgba(0, 0, 0, .08);
  border-radius: 6px;
  background: rgba(0, 0, 0, .025);
  padding: 8px 10px;
  display: flex;
  align-items: center;
  gap: 8px;
}

:global(.manager-resource-item span) {
  min-width: 0;
  flex: 1;
  display: grid;
  gap: 3px;
}

:global(.manager-resource-item b),
:global(.manager-about-card b) {
  min-width: 0;
  color: #4d4d4d;
  font-size: 15px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

:global(.manager-resource-item button),
:global(.manager-resource-item button.danger) {
  color: #df6763;
}

:global(.manager-about-card) {
  max-width: 520px;
  border: 1px solid rgba(0, 0, 0, .08);
  border-radius: 8px;
  background: rgba(57, 147, 147, .06);
  padding: 18px;
  display: flex;
  align-items: flex-start;
  gap: 14px;
}

:global(.manager-about-card svg) {
  color: var(--book-manager-primary, #399393);
  flex: 0 0 auto;
}

:global(.manager-about-card span) {
  display: grid;
  gap: 6px;
}

@media (max-width: 760px) {
  :global(.manager-settings-shell) {
    min-height: 620px;
    grid-template-columns: 1fr;
  }

  :global(.manager-settings-sidebar) {
    max-height: 210px;
    border-right: none;
    border-bottom: 1px solid rgba(0, 0, 0, .08);
  }

  :global(.manager-settings-main) {
    padding: 22px 20px 24px;
  }

  :global(.manager-theme-list) {
    grid-template-columns: repeat(2, minmax(120px, 1fr));
  }
}

/* 暗色主题适配 */
body[arco-theme='dark'] :global(.manager-settings-modal .arco-modal-content),
body[arco-theme='dark'] :global(.arco-modal.manager-settings-modal .arco-modal-content) {
  background: #23232e;
}
body[arco-theme='dark'] :global(.manager-settings-shell) {
  color: #d0d0d0;
  background: linear-gradient(135deg, rgba(57, 147, 147, .1), transparent 42%), #23232e;
}
body[arco-theme='dark'] :global(.manager-settings-sidebar) {
  border-right-color: rgba(255, 255, 255, .08);
}
body[arco-theme='dark'] :global(.manager-settings-sidebar h2),
body[arco-theme='dark'] :global(.manager-settings-main h2) {
  color: #e0e0e0;
}
body[arco-theme='dark'] :global(.manager-settings-tab) {
  color: #999;
}
body[arco-theme='dark'] :global(.manager-settings-tab.active) {
  background: var(--book-manager-primary, #399393);
  color: #fff;
}
body[arco-theme='dark'] :global(.manager-settings-divider) {
  border-top-color: rgba(255, 255, 255, .08);
}
body[arco-theme='dark'] :global(.manager-settings-close) {
  color: #b0b0b0;
}
body[arco-theme='dark'] :global(.manager-settings-close:hover) {
  background: rgba(255, 255, 255, .08);
}
body[arco-theme='dark'] :global(.manager-setting-row b),
body[arco-theme='dark'] :global(.manager-setting-field > span),
body[arco-theme='dark'] :global(.manager-setting-title) {
  color: #d0d0d0;
}
body[arco-theme='dark'] :global(.manager-setting-row em),
body[arco-theme='dark'] :global(.manager-setting-note),
body[arco-theme='dark'] :global(.manager-resource-item em),
body[arco-theme='dark'] :global(.manager-about-card em) {
  color: #999;
}
body[arco-theme='dark'] :global(.manager-setting-field input),
body[arco-theme='dark'] :global(.manager-setting-field select),
body[arco-theme='dark'] :global(.manager-setting-field textarea),
body[arco-theme='dark'] :global(.manager-parser-add input),
body[arco-theme='dark'] :global(.manager-background-add input) {
  border-color: rgba(255, 255, 255, .12);
  background: rgba(255, 255, 255, .06);
  color: #d0d0d0;
}
body[arco-theme='dark'] :global(.manager-setting-field select) {
  background: #2a2a36;
}
body[arco-theme='dark'] :global(.manager-resource-item) {
  border-color: rgba(255, 255, 255, .08);
}
body[arco-theme='dark'] :global(.manager-resource-item b) {
  color: #d0d0d0;
}
body[arco-theme='dark'] :global(.manager-about-card) {
  background: rgba(57, 147, 147, .08);
  border-color: rgba(255, 255, 255, .08);
}
body[arco-theme='dark'] :global(.manager-about-card b) {
  color: #d0d0d0;
}
body[arco-theme='dark'] :global(.manager-settings-scroll) {
  scrollbar-color: rgba(255, 255, 255, .2) transparent;
}

.book-sidebar {
  height: 100%;
  width: 168px;
  flex: 0 0 168px;
  padding: 16px 10px;
  border-right: 1px solid var(--color-border);
  background: var(--color-bg-2);
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  transition: width .22s ease, flex-basis .22s ease;
}

.book-sidebar.collapsed {
  width: 52px;
  flex: 0 0 52px;
  padding: 16px 6px;
}

.book-sidebar.collapsed .book-brand-text { display: none; }
.book-sidebar.collapsed .book-nav-item span { display: none; }
.book-sidebar.collapsed .book-nav-item { justify-content: center; padding: 0; }
.book-sidebar.collapsed .library-scan-panel { display: none; }

.book-sidebar-toggle {
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--color-text-3);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
}
.book-sidebar-toggle:hover { background: var(--color-fill-2); color: var(--color-text-1); }

.book-sidebar-divider {
  margin: 12px 4px;
  height: 1px;
  background: var(--color-border);
}

.book-brand-text { transition: opacity .15s ease; }

.book-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 8px 18px;
}

.book-brand-title {
  font-size: 18px;
  font-weight: 700;
}

.book-brand-sub {
  margin-top: 2px;
  color: var(--color-text-3);
  font-size: 12px;
}

.book-nav-item {
  width: 100%;
  height: 34px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--color-text-2);
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 0 10px;
  cursor: pointer;
}

.book-nav-item.active,
.book-nav-item:hover {
  background: var(--color-fill-2);
  color: var(--book-manager-primary, var(--color-text-1));
}

.book-main {
  position: relative;
  min-width: 0;
  min-height: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  margin: 18px 18px 18px 14px;
}

.book-header {
  padding: 12px 18px 10px;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.book-header-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.book-header-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.book-header h2 {
  margin: 0;
  font-size: 20px;
  line-height: 1.25;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.book-header p {
  margin: 2px 0 0;
  color: var(--color-text-3);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.book-header-ctrl-row {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.book-total-count {
  color: var(--color-text-3);
  font-size: 12px;
  white-space: nowrap;
  flex-shrink: 0;
}

.book-card-scale {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}
.book-card-scale-slider {
  width: 100px;
}

.book-header-divider {
  width: 1px;
  height: 16px;
  background: var(--color-border);
  flex-shrink: 0;
}

.book-status-filter {
  height: 28px;
  border: 1px solid var(--color-border);
  border-radius: 5px;
  background: var(--color-bg-1);
  color: var(--color-text-1);
  font-size: 12px;
  padding: 0 6px;
  cursor: pointer;
  flex-shrink: 0;
}

.book-search {
  width: 200px;
  flex-shrink: 0;
}

.book-sort-btn {
  min-width: 100px;
  flex-shrink: 0;
}

.book-sort-dropdown {
  min-width: 140px;
  padding: 6px 0;
}

.book-sort-section-title {
  padding: 2px 14px 4px;
  font-size: 11px;
  color: var(--color-text-3);
}

.book-sort-item {
  padding: 5px 14px;
  font-size: 13px;
  color: var(--color-text-1);
  cursor: pointer;
}

.book-sort-item:hover {
  background: var(--color-fill-2);
}

.book-sort-item.active {
  color: rgb(var(--primary-6));
  font-weight: 500;
}

.book-sort-divider {
  height: 1px;
  background: var(--color-border);
  margin: 4px 0;
}

.book-view-toggle {
  flex: 0 0 auto;
}

.book-view-toggle :deep(.arco-radio-button) {
  padding: 0 9px;
}

.book-view-toggle :deep(.arco-radio-button-content) {
  display: inline-flex;
  align-items: center;
}

.book-drive-btn {
  min-width: 84px;
  flex-shrink: 0;
}

.book-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.book-content {
  min-height: 0;
  flex: 1;
  padding: 18px;
  overflow: auto;
}

.book-batch-bar {
  position: sticky;
  top: -18px;
  z-index: 5;
  min-height: 42px;
  margin: -4px 0 14px;
  padding: 8px 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-2);
  box-shadow: 0 8px 22px rgba(0, 0, 0, .12);
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.book-batch-bar > span {
  color: var(--color-text-2);
  font-size: 12px;
  font-weight: 600;
}

.book-batch-close {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 5px;
  background: var(--color-fill-2);
  color: var(--color-text-2);
  display: grid;
  place-items: center;
  cursor: pointer;
}

.book-trash-toolbar {
  min-height: 38px;
  margin: -2px 0 14px;
  padding: 7px 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-2);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.book-trash-toolbar > span {
  min-width: 0;
  color: var(--color-text-2);
  font-size: 12px;
  font-weight: 600;
}

.book-card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(calc(133px * var(--card-scale, 1)), 1fr));
  gap: 12px;
}

.book-card-item {
  position: relative;
  min-width: 0;
  border: 1px solid transparent;
  border-radius: 6px;
  cursor: pointer;
  transition: border-color .12s ease;
}
.book-card-item:hover { border-color: var(--color-border); }
.book-card-item.multi-selected { outline: 2px solid rgb(var(--primary-6)); outline-offset: 2px; }

.book-card-item-cover {
  position: relative;
  width: calc(105px * var(--card-scale, 1));
  height: calc(137px * var(--card-scale, 1));
  margin: 10px auto 6px;
  border-radius: 2px;
  background: var(--cover-color);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  box-shadow: 0 4px 8px rgba(0, 0, 0, .12);
}
.book-card-item-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: 2px;
}
.book-card-item-format {
  font-size: 14px;
  font-weight: 700;
  color: rgba(255, 255, 255, .9);
  text-shadow: 0 1px 3px rgba(0, 0, 0, .3);
}

.book-card-select {
  position: absolute;
  left: 4px;
  top: 4px;
  z-index: 3;
  background: rgba(255, 255, 255, .88);
  color: var(--color-text-3);
}
.book-card-select.checked { color: #fff; background: rgb(var(--primary-6)); }

.book-card-item-favorite {
  position: absolute;
  right: 4px;
  top: 4px;
  z-index: 3;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: rgba(255, 255, 255, .8);
  color: rgb(var(--danger-6));
  display: grid;
  place-items: center;
}

.book-card-item-title {
  width: 80%;
  margin: 0 auto;
  height: 31px;
  font-size: 12px;
  line-height: 15px;
  text-align: left;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  color: var(--color-text-1);
}

.book-card-item-progress {
  display: block;
  margin: 4px auto 8px;
  width: 80%;
  font-size: 11px;
  color: var(--color-text-3);
  text-align: left;
}

.book-shelf-books {
  min-height: 192px;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(calc(96px * var(--card-scale, 1)), 1fr));
  align-items: end;
  gap: calc(14px * var(--card-scale, 1));
  padding: 0 10px;
}

.book-shelf-board {
  height: calc(14px * var(--card-scale, 1));
  border-radius: 3px;
  background: linear-gradient(180deg, #9b7752, #6f4f34);
  box-shadow: 0 9px 18px rgba(31, 22, 14, 0.24);
}

.book-card {
  position: relative;
  height: calc(176px * var(--card-scale, 1));
  border: none;
  border-radius: 5px 8px 8px 5px;
  background: var(--book-color);
  color: #fff;
  padding: 12px 9px 10px 17px;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  text-align: left;
  cursor: pointer;
  box-shadow: 0 10px 18px rgba(0, 0, 0, 0.22);
  transform-origin: bottom center;
  transition: transform .22s ease, box-shadow .22s ease, opacity .22s ease;
  overflow: hidden;
}

.book-card:hover {
  transform: translateY(-8px) rotate(-1deg);
  box-shadow: 0 18px 30px rgba(0, 0, 0, 0.26);
}

.book-card.taking {
  animation: takeBook .52s cubic-bezier(.2, .8, .2, 1);
}

.book-card.returning {
  animation: returnBook .52s cubic-bezier(.2, .8, .2, 1);
}

.book-card.selected {
  opacity: .42;
}

.book-card.multi-selected {
  outline: 2px solid rgb(var(--primary-6));
  outline-offset: 2px;
}
.book-list-item.multi-selected {
  outline: 2px solid rgb(var(--primary-6));
  outline-offset: 2px;
}



.book-cover {
  height: 72px;
  border-radius: 4px;
  display: grid;
  place-items: center;
  background: rgba(255, 255, 255, .16);
  overflow: hidden;
}

.book-cover img,
.book-list-cover img,
.book-detail-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.book-list-cover-format {
  font-size: 14px;
  font-weight: 700;
  color: rgba(255, 255, 255, .9);
  text-shadow: 0 1px 3px rgba(0, 0, 0, .3);
}

.book-cover-art {
  aspect-ratio: 2 / 3;
  width: 100%;
  border-radius: 5px;
  display: grid;
  place-items: center;
  color: #fff;
  background: var(--book-color);
  overflow: hidden;
  box-shadow: 0 12px 22px rgba(0, 0, 0, .18);
}

.book-cover-art-format {
  font-size: 22px;
  font-weight: 700;
  color: rgba(255, 255, 255, .92);
  text-shadow: 0 1px 3px rgba(0, 0, 0, .3);
}

.book-cover-format-label {
  font-size: 16px;
  font-weight: 700;
  color: rgba(255, 255, 255, .88);
  text-shadow: 0 1px 3px rgba(0, 0, 0, .3);
}

.book-title {
  margin-top: 10px;
  font-weight: 700;
  font-size: 13px;
  line-height: 1.35;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.book-author {
  margin-top: auto;
  font-size: 11px;
  color: rgba(255, 255, 255, .76);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}










.book-engine-chip,
.book-progress-chip {
  display: inline-block;
  margin-left: 6px;
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 11px;
  color: var(--color-text-2);
  background: var(--color-fill-2);
}


.book-select-button {
  border: none;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  cursor: pointer;
  font-size: 13px;
  opacity: 0;
  transition: opacity .15s ease, background .15s ease;
}

.book-select-button.checked,
.book-card.multi-selected .book-select-button {
  opacity: 1;
}

.book-select-button.checked {
  border-color: rgb(var(--primary-6));
  background: rgb(var(--primary-6));
}

.book-card-favorite {
  position: absolute;
  right: 7px;
  top: 7px;
  z-index: 2;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: rgba(255, 255, 255, .2);
  color: rgba(255, 255, 255, .82);
}

.book-favorite-button.active {
  color: rgb(var(--danger-6));
}
.book-card-favorite.active {
  background: rgba(255, 255, 255, .88);
}

.book-groups {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 10px;
}

.book-group {
  position: relative;
  height: 58px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-2);
  color: var(--color-text-1);
  padding: 0 12px;
  display: flex;
  align-items: center;
  gap: 10px;
  cursor: pointer;
}

.book-group.selected {
  border-color: rgb(var(--primary-6));
  box-shadow: 0 0 0 1px rgba(var(--primary-6), .22);
}

.book-group.wide {
  grid-column: span 2;
}

.book-group span {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.book-group span em,
.book-group span small {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  font-style: normal;
}

.book-group span small {
  color: var(--color-text-3);
  font-size: 11px;
}

.book-group b {
  color: var(--color-text-3);
}

.book-group:hover,
.book-list-item:hover,
.book-annotation-item:hover {
  background: var(--color-fill-2);
}

.book-folder-check {
  width: 22px;
  height: 22px;
  border: 1px solid var(--color-border-3);
  border-radius: 50%;
  background: var(--color-bg-1);
  color: #fff;
  display: grid;
  place-items: center;
  font-size: 13px;
  cursor: pointer;
}

.book-folder-check.checked {
  border-color: rgb(var(--primary-6));
  background: rgb(var(--primary-6));
}

.book-group-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
  color: var(--color-text-3);
}

.book-source-pill {
  max-width: 260px;
  padding: 3px 8px;
  border-radius: 5px;
  background: var(--color-fill-2);
  color: var(--color-text-2);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.book-annotation-list {
  display: grid;
  gap: 10px;
}

.book-annotation-toolbar {
  margin: -2px 0 14px;
  padding: 8px 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-2);
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.book-annotation-tags {
  min-width: 0;
  flex: 1 1 260px;
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.book-annotation-tags > span,
.book-annotation-tags > em {
  color: var(--color-text-3);
  font-size: 12px;
  font-style: normal;
}

.book-annotation-tag {
  min-height: 24px;
  max-width: 140px;
  padding: 0 8px;
  border: 1px solid var(--color-border);
  border-radius: 5px;
  background: var(--color-fill-1);
  color: var(--color-text-2);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.book-annotation-tag.active {
  border-color: rgb(var(--primary-6));
  background: rgb(var(--primary-1));
  color: rgb(var(--primary-6));
}

.book-annotation-select {
  width: 160px;
  max-width: 100%;
  height: 28px;
  border: 1px solid var(--color-border);
  border-radius: 5px;
  background: var(--color-bg-1);
  color: var(--color-text-1);
  font-size: 12px;
}

.book-annotation-global-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.book-annotation-item {
  min-width: 0;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-2);
  color: var(--color-text-1);
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px;
  text-align: left;
  cursor: pointer;
  transition: background .16s ease, opacity .16s ease;
}

.book-annotation-item.disabled {
  cursor: default;
  opacity: .55;
}

.book-annotation-item.disabled:hover {
  background: var(--color-bg-2);
}

.book-annotation-item > svg {
  flex: 0 0 auto;
  margin-top: 2px;
  color: var(--color-text-3);
}

.book-annotation-item > span:not(.book-annotation-actions) {
  min-width: 0;
  flex: 1 1 auto;
  display: grid;
  gap: 5px;
}

.book-annotation-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
}

.book-annotation-actions > span {
  width: 28px;
  height: 28px;
  border-radius: 5px;
  color: var(--color-text-3);
  display: grid;
  place-items: center;
  cursor: pointer;
}

.book-annotation-actions > span:hover {
  background: var(--color-fill-3);
  color: rgb(var(--primary-6));
}

.book-annotation-item b,
.book-annotation-item em,
.book-annotation-item small {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.book-annotation-item b {
  font-size: 13px;
}

.book-annotation-item em {
  color: var(--color-text-1);
  font-style: normal;
  white-space: nowrap;
}

.book-annotation-item small {
  color: var(--color-text-3);
  font-size: 12px;
  white-space: nowrap;
}

.book-annotation-item-tags {
  display: flex;
  gap: 5px;
  flex-wrap: wrap;
  white-space: normal;
}

.book-annotation-item-tags i {
  max-width: 120px;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--color-fill-2);
  color: var(--color-text-2);
  font-size: 11px;
  font-style: normal;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  gap: 10px;
}

.book-list-linear {
  grid-template-columns: minmax(0, 1fr);
}

.book-list-item {
  min-width: 0;
  height: 78px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-2);
  color: var(--color-text-1);
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px;
  text-align: left;
  cursor: pointer;
}

.book-list-cover {
  width: 48px;
  height: 62px;
  flex: 0 0 48px;
  border-radius: 4px;
  color: #fff;
  display: grid;
  place-items: center;
  overflow: hidden;
}

.book-list-meta {
  min-width: 0;
  flex: 1;
}

.book-list-title {
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-list-sub {
  margin-top: 6px;
  color: var(--color-text-3);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-list-item.trash-item {
  cursor: default;
}

.book-trash-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 8px;
}

.book-favorite-button {
  width: 30px;
  height: 30px;
  flex: 0 0 30px;
  border-radius: 6px;
  background: var(--color-fill-2);
}

.book-cover-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.book-cover-item {
  position: relative;
  display: flex;
  gap: 16px;
  min-width: 0;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-bg-2);
  padding: 14px;
  cursor: pointer;
  transition: background .16s ease, border-color .16s ease;
}
.book-cover-item:hover { border-color: var(--color-primary-light-3); background: var(--color-fill-1); }
.book-cover-item.multi-selected { outline: 2px solid rgb(var(--primary-6)); outline-offset: 2px; }

.book-cover-item-header {
  position: absolute;
  right: 14px;
  top: 14px;
  display: flex;
  align-items: center;
  gap: 6px;
  z-index: 2;
}
.book-cover-item-progress {
  padding: 2px 8px;
  border-radius: 4px;
  background: rgba(var(--primary-6), .14);
  color: rgb(var(--primary-6));
  font-size: 12px;
  font-weight: 600;
}
.book-cover-item-favorite { color: rgb(var(--danger-6)); display: flex; }

.book-cover-select {
  position: absolute;
  left: 14px;
  top: 14px;
  z-index: 3;
}
.book-cover-select.checked { color: #fff; background: rgb(var(--primary-6)); }

.book-cover-item-cover {
  width: 120px;
  height: 170px;
  flex: 0 0 120px;
  border-radius: 4px;
  display: grid;
  place-items: center;
  overflow: hidden;
  box-shadow: 0 14px 24px rgba(0, 0, 0, .2);
}
.book-cover-item-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.book-cover-item-cover-format {
  font-size: 26px;
  font-weight: 800;
  color: rgba(255, 255, 255, .92);
  text-shadow: 0 1px 4px rgba(0, 0, 0, .3);
}

.book-cover-item-body {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 5px;
}
.book-cover-item-title {
  font-size: 15px;
  font-weight: 600;
  line-height: 1.35;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.book-cover-item-author,
.book-cover-item-publisher {
  font-size: 13px;
  color: var(--color-text-2);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.book-cover-item-desc {
  flex: 1;
  min-height: 0;
}
.book-cover-item-desc-text {
  font-size: 13px;
  color: var(--color-text-2);
  line-height: 1.55;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.book-cover-item-meta {
  display: flex;
  gap: 10px;
  font-size: 12px;
  color: var(--color-text-3);
}

.book-cover-art-format {
  font-size: 22px;
  font-weight: 700;
  color: rgba(255, 255, 255, .92);
  text-shadow: 0 1px 3px rgba(0, 0, 0, .3);
}

.book-card > .book-select-button {
  position: absolute;
  left: 7px;
  top: 7px;
  z-index: 3;
  background: rgba(255, 255, 255, .88);
  color: var(--color-text-3);
}
.book-card > .book-select-button.checked {
  color: #fff;
  background: rgb(var(--primary-6));
}

.book-cover-art {
  aspect-ratio: 2 / 3;
  width: 100%;
  border-radius: 5px;
  display: grid;
  place-items: center;
  color: #fff;
  background: var(--book-color);
  overflow: hidden;
  box-shadow: 0 12px 22px rgba(0, 0, 0, .18);
}

.book-cover-art img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.book-cover-title {
  margin-top: 9px;
  font-weight: 700;
  line-height: 1.35;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.book-cover-author {
  margin-top: 4px;
  color: var(--color-text-3);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-cover-publisher {
  margin-top: 2px;
  color: var(--color-text-4);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-cover-progress {
  position: absolute;
  right: 10px;
  top: 10px;
  padding: 2px 7px;
  border-radius: 4px;
  background: rgba(0, 0, 0, .48);
  color: #fff;
  font-size: 11px;
  font-weight: 600;
  z-index: 2;
}

.book-detail {
  width: 0;
  flex: 0 0 0;
  border-left: 1px solid var(--color-border);
  background: var(--color-bg-2);
  overflow: hidden;
  transition: width .28s ease, flex-basis .28s ease;
}

.book-detail.visible {
  width: 286px;
  flex-basis: 286px;
  padding: 18px;
}

.book-detail-close {
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 6px;
  background: var(--color-fill-2);
  color: var(--color-text-2);
  display: grid;
  place-items: center;
  margin-left: auto;
  cursor: pointer;
}

.book-detail-cover {
  width: 154px;
  height: 210px;
  margin: 12px auto 18px;
  border-radius: 6px;
  color: #fff;
  display: grid;
  place-items: center;
  overflow: hidden;
  box-shadow: 0 18px 36px rgba(0, 0, 0, .24);
  animation: detailBookIn .34s ease;
}
.book-detail-cover-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 16px;
  text-align: center;
}
.book-detail-cover-format {
  font-size: 28px;
  font-weight: 800;
  color: rgba(255, 255, 255, .92);
  text-shadow: 0 1px 4px rgba(0, 0, 0, .3);
}
.book-detail-cover-title {
  font-size: 12px;
  font-weight: 500;
  color: rgba(255, 255, 255, .8);
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}

.book-detail h3 {
  margin: 0;
  font-size: 18px;
  line-height: 1.35;
}

.book-detail-author {
  margin-top: 8px;
  color: var(--color-text-2);
  font-weight: 600;
}

.book-detail p {
  margin: 14px 0;
  color: var(--color-text-2);
  font-size: 13px;
  line-height: 1.7;
}

.book-detail-grid {
  display: grid;
  grid-template-columns: 46px minmax(0, 1fr);
  gap: 8px 10px;
  font-size: 12px;
}

.book-detail-actions {
  display: grid;
  gap: 8px;
  margin: 14px 0 4px;
}

.book-detail-grid span {
  color: var(--color-text-3);
}

.book-detail-grid b {
  min-width: 0;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-subjects {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 14px;
}

.book-subjects span {
  max-width: 100%;
  padding: 3px 7px;
  border-radius: 4px;
  background: var(--color-fill-2);
  color: var(--color-text-2);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-folder-menu {
  min-width: 170px;
  padding: 4px;
}

.book-folder-menu-item {
  width: 100%;
  height: 30px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--color-text-1);
  text-align: left;
  padding: 0 10px;
  cursor: pointer;
}

.book-folder-menu-item:hover {
  background: var(--color-fill-2);
}

.book-folder-menu-item.danger {
  color: rgb(var(--danger-6));
}

.book-folder-menu-item:disabled {
  color: var(--color-text-4);
  cursor: not-allowed;
  background: transparent;
}

.book-tag-editor {
  margin: 8px 0 12px;
  padding: 10px 12px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-fill-1);
}

.book-tag-editor-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 10px;
}

.book-tag-editor-item {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 3px 6px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-1);
  font-size: 12px;
}

.book-tag-editor-name {
  min-width: 20px;
}

.book-tag-editor-input {
  width: 80px;
  height: 22px;
  border: 1px solid var(--color-primary-light-3);
  border-radius: 3px;
  padding: 0 4px;
  font-size: 12px;
  background: var(--color-bg-1);
  color: var(--color-text-1);
}

.btn-tag-rename,
.btn-tag-delete {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--color-text-3);
  cursor: pointer;
  padding: 0;
}

.btn-tag-rename:hover { color: var(--color-primary-light-3); }
.btn-tag-delete:hover { color: rgb(var(--danger-6)); }

.book-tag-editor-add {
  display: flex;
  align-items: center;
  gap: 6px;
}

.book-tag-editor-add .book-tag-editor-input {
  width: 140px;
}

.btn-tag-manage {
  margin-left: 4px;
}

@keyframes takeBook {
  0% { transform: translateY(0) scale(1); }
  55% { transform: translateY(-28px) scale(1.06) rotate(-4deg); }
  100% { transform: translateX(34px) translateY(-10px) scale(.92); opacity: .38; }
}

@keyframes returnBook {
  0% { transform: translateX(34px) translateY(-10px) scale(.92); opacity: .38; }
  70% { transform: translateY(-18px) scale(1.04) rotate(3deg); opacity: 1; }
  100% { transform: translateY(0) scale(1); }
}

@keyframes detailBookIn {
  from { transform: translateX(-26px) rotate(-4deg); opacity: .2; }
  to { transform: translateX(0) rotate(0); opacity: 1; }
}
</style>
