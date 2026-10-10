import { expect, test } from './fixtures/boxPlayer'

test.describe.configure({ timeout: 90_000 })

const serverId = 'media-server-theme-e2e'

const svgData = (label: string, from: string, to: string, width = 1280, height = 720) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(`
  <svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
    <defs>
      <linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient>
      <radialGradient id="r" cx="34%" cy="28%"><stop stop-color="#f5dfd0" stop-opacity=".58"/><stop offset="1" stop-color="#20242e" stop-opacity="0"/></radialGradient>
    </defs>
    <rect width="100%" height="100%" fill="url(#g)"/><rect width="100%" height="100%" fill="url(#r)"/>
    <circle cx="${width * 0.72}" cy="${height * 0.38}" r="${height * 0.28}" fill="#dad7ff" opacity=".12"/>
    <text x="${width * 0.08}" y="${height * 0.78}" fill="white" opacity=".88" font-family="sans-serif" font-size="${Math.max(28, height * 0.08)}" font-weight="700">${label}</text>
  </svg>
`)}`

const mediaCards = [
  { id: 'video-0', kind: 'video', title: 'H.264 1080p', selected: true, streamIndex: 0, rows: [{ label: '分辨率', value: '1920x1080' }, { label: 'Video range', value: 'SDR' }, { label: '比特率', value: '5.3 Mbps' }] },
  { id: 'audio-1', kind: 'audio', title: 'AAC 2.0', selected: true, streamIndex: 1, rows: [{ label: 'Channels', value: '2' }] }
]

const people = Array.from({ length: 10 }, (_, index) => ({
  id: `person-${index + 1}`,
  name: ['顾北辰', '林初夏', '沈砚', '程星河', '江晚宁', '周予安', '陆清和', '苏明月', '叶知秋', '许长风'][index],
  role: ['领衔主演', '主演', '主演', '特别出演', '导演', '编剧', '摄影', '制片人', '配乐', '剪辑'][index],
  image: svgData(String(index + 1), ['#403a5f', '#46566f', '#5c443d'][index % 3], '#181a22', 200, 200)
}))

const series = {
  id: 'series-reference-detail',
  serverId,
  provider: 'emby',
  kind: 'series',
  rawType: 'Series',
  title: '光影纪事',
  overview: '在一座被群山与云海环抱的小城里，几位年轻人因一卷遗失的旧胶片相遇，并在追寻影像来源的过程中重新理解彼此、故乡与时间。',
  year: 2026,
  runtimeMinutes: 42,
  rating: 8.7,
  officialRating: 'PG-13',
  images: {
    primary: svgData('光影纪事', '#4b405f', '#171923', 800, 1200),
    backdrop: svgData('光影纪事', '#556c8c', '#211b2a'),
    logo: svgData('光影纪事', '#292331', '#141419', 600, 240)
  },
  genres: ['剧情', '奇幻', '悬疑'],
  studios: ['云间影业'],
  people,
  productionLocations: ['中国'],
  externalLinks: [],
  mediaInfoCards: mediaCards,
  sourceOptions: [{ id: 'source-main', title: '1080p', fileLabel: 'S01E01 - 光影纪事 - 第一集.mp4', fileSubLabel: '1.5 GB  H.264（1080p）  AAC 2.0  5.3 Mbps  25 fps', mediaInfoCards: mediaCards }],
  fileLabel: 'S01E01 - 光影纪事 - 第一集.mp4',
  fileSubLabel: '1.5 GB  H.264（1080p）  AAC 2.0  5.3 Mbps  25 fps',
  isPlayed: false,
  isFavorite: false
}

const season = {
  id: 'season-1', serverId, provider: 'emby', kind: 'season', rawType: 'Season', title: '第 1 季', seriesId: series.id, seasonNumber: 1, images: {}
}

const episodes = Array.from({ length: 10 }, (_, index) => ({
  id: `episode-${index + 1}`,
  serverId,
  provider: 'emby',
  kind: 'episode',
  rawType: 'Episode',
  title: ['旧胶片', '山城来信', '云海之上', '暗房微光', '失落的画面', '重逢', '无声放映', '回到原点', '时间的背面', '最后一卷'][index],
  parentTitle: series.title,
  seriesId: series.id,
  seasonNumber: 1,
  episodeNumber: index + 1,
  runtimeMinutes: 42,
  year: 2026,
  rating: 8.4 + index / 20,
  overview: `第 ${index + 1} 集：众人在新的线索中继续追寻胶片背后的故事，也逐渐发现这段影像与每个人的过去密切相关。`,
  images: {
    thumb: svgData(`第 ${index + 1} 集`, ['#3d536b', '#624858', '#4b5d47'][index % 3], '#17191f', 640, 360),
    backdrop: series.images.backdrop
  },
  genres: series.genres,
  studios: series.studios,
  people,
  productionLocations: series.productionLocations,
  externalLinks: [],
  mediaInfoCards: mediaCards,
  sourceOptions: series.sourceOptions,
  fileLabel: `S01E${String(index + 1).padStart(2, '0')} - 光影纪事.mp4`,
  fileSubLabel: series.fileSubLabel,
  isPlayed: index === 0,
  isFavorite: false
}))

test('media detail matches the compact cinematic reference layout', async ({ boxPlayer }, testInfo) => {
  const { page, pageErrors } = boxPlayer
  await page.setViewportSize({ width: 1600, height: 900 })

  await page.evaluate(({ serverId }) => {
    const now = Date.now()
    const server = {
      id: serverId,
      type: 'emby',
      name: 'My Emby',
      baseUrl: 'http://127.0.0.1:65530',
      accessToken: 'e2e-token',
      userId: 'e2e-user',
      deviceId: 'e2e-device',
      loginStatus: 'success',
      createdAt: now,
      updatedAt: now,
      lastUsedAt: now
    }
    localStorage.setItem('MediaServer_Registry', JSON.stringify([server]))
    localStorage.setItem('MediaServer_Preferences', JSON.stringify({ currentServerId: server.id, serverListView: 'grid', serverSortBy: 'lastUsedAt', serverSortOrder: 'desc', serverSearchText: '', pinnedServerIds: [] }))
  }, { serverId })
  await page.reload()
  await page.waitForLoadState('domcontentloaded')
  const loginDialog = page.locator('.userloginmodal')
  await loginDialog.waitFor({ state: 'visible', timeout: 3_000 }).catch(() => undefined)
  if (await loginDialog.isVisible().catch(() => false)) {
    await loginDialog.getByRole('button', { name: 'Close' }).click({ force: true }).catch(() => undefined)
    await loginDialog.waitFor({ state: 'hidden', timeout: 5_000 }).catch(() => undefined)
  }
  await page.addStyleTag({ content: '.userloginmodal, .arco-modal-mask { display: none !important; pointer-events: none !important; } #xbybody { display: block !important; }' })

  if (!await page.locator('body').getAttribute('arco-theme')) await page.locator('#xbyhead2 button:has(svg.iconday)').dispatchEvent('click')
  await expect(page.locator('body')).toHaveAttribute('arco-theme', 'dark')

  await page.evaluate(() => {
    const app = (document.querySelector('#app') as HTMLElement & { __vue_app__?: any }).__vue_app__
    app?.config.globalProperties.$pinia?._s.get('app')?.toggleTab('media-server')
  })
  await expect(page.locator('.media-server-sidebar .server-item').filter({ hasText: 'My Emby' })).toHaveCount(1)
  await page.evaluate(({ series, season, episodes }) => {
    const app = (document.querySelector('#app') as HTMLElement & { __vue_app__?: any }).__vue_app__
    const pinia = app?.config.globalProperties.$pinia
    if (!pinia) throw new Error('Pinia is unavailable in the Electron renderer')
    const content = pinia._s.get('media-server-content')
    const navigation = pinia._s.get('media-server-navigation')
    if (!content || !navigation) throw new Error('Media-server stores are unavailable')
    const seriesKey = `${series.serverId}:${series.id}`
    const seasonKey = `${series.serverId}:${season.id}`
    content.homeData[series.serverId] = { resume: [], latest: [series], latestTotal: 1, nextUp: [], nextUpTotal: 0, libraries: [], statistics: { libraryCount: 0, movieCount: 0, seriesCount: 1, episodeCount: episodes.length } }
    content.itemDetails[seriesKey] = series
    content.libraryPages[seriesKey] = [season]
    content.libraryPagedPages[seriesKey] = { key: seriesKey, items: [season], total: 1, currentPage: 0, hasNextPage: false }
    content.libraryPages[seasonKey] = episodes
    content.libraryPagedPages[seasonKey] = { key: seasonKey, items: episodes, total: episodes.length, currentPage: 0, hasNextPage: false }
    for (const episode of episodes) content.itemDetails[`${series.serverId}:${episode.id}`] = episode
    content.loadingHome = false
    content.homeError = ''
    content.detailError = ''
    navigation.goHome()
    navigation.push({ kind: 'item-detail', itemId: series.id, title: series.title })
  }, { series, season, episodes })

  const shell = page.locator('.detail-shell')
  await expect(shell).toBeVisible({ timeout: 30_000 })
  await expect.poll(() => page.locator('.workspace-page').evaluate((element) => element.scrollTop)).toBe(0)
  await expect(page.locator('.detail-top-back')).toContainText(series.title)
  await expect(page.locator('.detail-primary-play')).toBeVisible()
  await expect(page.locator('.detail-icon-actions .detail-square-action')).toHaveCount(3)
  await expect(page.getByTitle('加入播放列表')).toHaveCount(0)
  await expect(page.locator('.detail-play-version-trigger')).toBeVisible()
  await expect(page.locator('.detail-score-logo')).toBeVisible()
  await expect(page.locator('.detail-score-logo')).toHaveAttribute('alt', 'TMDB')
  await expect(page.locator('.detail-episode-card')).toHaveCount(episodes.length)
  await expect(page.locator('.detail-section-people .person-card')).toHaveCount(people.length)
  await expect(page.locator('.detail-hero-poster')).toHaveCount(0)

  const stageBox = await page.locator('.detail-backdrop-stage').boundingBox()
  const lowerBox = await page.locator('.detail-lower-content').boundingBox()
  expect(stageBox).not.toBeNull()
  expect(lowerBox).not.toBeNull()
  expect(stageBox!.height).toBeGreaterThanOrEqual(620)
  expect(Math.abs((stageBox!.y + stageBox!.height) - lowerBox!.y)).toBeLessThanOrEqual(2)
  const visibleBackdropRatio = (stageBox!.height - 54) / (stageBox!.width * 9 / 16)
  expect(visibleBackdropRatio).toBeGreaterThan(0.5)

  const heroActionsBox = await page.locator('.detail-actions-column').boundingBox()
  const playRowBox = await page.locator('.detail-play-row').boundingBox()
  const playButtonBox = await page.locator('.detail-primary-play').boundingBox()
  const versionButtonBox = await page.locator('.detail-play-version-trigger').boundingBox()
  const actionButtons = page.locator('.detail-icon-actions .detail-square-action')
  const actionRowBox = await page.locator('.detail-icon-actions').boundingBox()
  const firstActionBox = await actionButtons.first().boundingBox()
  const secondActionBox = await actionButtons.nth(1).boundingBox()
  const lastActionBox = await actionButtons.last().boundingBox()
  expect(heroActionsBox).not.toBeNull()
  expect(playRowBox).not.toBeNull()
  expect(playButtonBox).not.toBeNull()
  expect(versionButtonBox).not.toBeNull()
  expect(actionRowBox).not.toBeNull()
  expect(firstActionBox).not.toBeNull()
  expect(secondActionBox).not.toBeNull()
  expect(lastActionBox).not.toBeNull()
  expect((stageBox!.y + stageBox!.height) - (heroActionsBox!.y + heroActionsBox!.height)).toBeLessThanOrEqual(36)
  expect(versionButtonBox!.x).toBeGreaterThan(playButtonBox!.x + playButtonBox!.width)
  expect(Math.abs(versionButtonBox!.y - playButtonBox!.y)).toBeLessThanOrEqual(1)
  expect(Math.abs(firstActionBox!.x - playRowBox!.x)).toBeLessThanOrEqual(1)
  expect(Math.abs(secondActionBox!.x - (firstActionBox!.x + firstActionBox!.width) - 10)).toBeLessThanOrEqual(1)
  expect(Math.abs(lastActionBox!.x - (secondActionBox!.x + secondActionBox!.width) - 10)).toBeLessThanOrEqual(1)
  expect(Math.abs(actionRowBox!.width - playRowBox!.width)).toBeLessThanOrEqual(1)
  expect(Math.abs(firstActionBox!.width - secondActionBox!.width)).toBeLessThanOrEqual(1)
  expect(Math.abs(secondActionBox!.width - lastActionBox!.width)).toBeLessThanOrEqual(1)
  expect(Math.abs((lastActionBox!.x + lastActionBox!.width) - (playRowBox!.x + playRowBox!.width))).toBeLessThanOrEqual(1)
  expect(await actionButtons.first().evaluate((element) => getComputedStyle(element).flexGrow)).toBe('1')

  const firstEpisodeBox = await page.locator('.detail-episode-card').first().boundingBox()
  expect(firstEpisodeBox?.width).toBeLessThanOrEqual(200)

  const selectedCover = page.locator('.detail-episode-card.selected .detail-episode-cover')
  await expect(selectedCover).toBeVisible()
  const selectedOutline = await selectedCover.evaluate((element) => getComputedStyle(element).outlineColor)
  expect(selectedOutline).toBe('rgb(255, 122, 0)')

  const personAvatar = page.locator('.detail-section-people .person-avatar').first()
  const avatarBox = await personAvatar.boundingBox()
  expect(avatarBox?.width).toBeCloseTo(72, 0)
  expect(avatarBox?.height).toBeCloseTo(72, 0)

  await page.screenshot({ path: testInfo.outputPath('media-detail-reference-layout.png'), fullPage: false })
  expect(pageErrors).toEqual([])
})
