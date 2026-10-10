import { expect, test } from './fixtures/boxPlayer'

test.describe.configure({ timeout: 90_000 })

const svgData = (label: string, from: string, to: string, width = 1280, height = 720) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(`
  <svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
    <defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient></defs>
    <rect width="100%" height="100%" fill="url(#g)"/>
    <circle cx="${width * 0.72}" cy="${height * 0.34}" r="${height * 0.3}" fill="#e5dbff" opacity=".18"/>
    <text x="${width * 0.08}" y="${height * 0.78}" fill="white" opacity=".9" font-family="sans-serif" font-size="${Math.max(28, height * 0.08)}" font-weight="700">${label}</text>
  </svg>
`)}`

const driveFiles = [
  {
    id: 'scraped-file-1080',
    name: 'S01E01.1080p.H264.AAC.mp4',
    path: '/media/S01E01.1080p.H264.AAC.mp4',
    driveId: 'local',
    driveServerId: 'local',
    fileSize: 1_500_000_000,
    height: 1080,
    videoDuration: '1440',
    subtitleFiles: [
      { id: 'subtitle-zh', name: 'S01E01.zh-CN.ass', path: '/media/S01E01.zh-CN.ass', driveId: 'local', driveServerId: 'local', fileSize: 128_000 },
      { id: 'subtitle-en', name: 'S01E01.en.srt', path: '/media/S01E01.en.srt', driveId: 'local', driveServerId: 'local', fileSize: 86_000 }
    ]
  },
  {
    id: 'scraped-file-4k',
    name: 'S01E01.2160p.HEVC.DV.mp4',
    path: '/media/S01E01.2160p.HEVC.DV.mp4',
    driveId: 'local',
    driveServerId: 'local',
    fileSize: 5_500_000_000,
    height: 2160,
    videoDuration: '24:00',
    subtitleFiles: [
      { id: 'subtitle-4k-zh', name: 'S01E01.2160p.zh-CN.ass', path: '/media/S01E01.2160p.zh-CN.ass', driveId: 'local', driveServerId: 'local', fileSize: 132_000 }
    ]
  }
]

const episodes = Array.from({ length: 8 }, (_, index) => ({
  id: index + 1,
  episodeNumber: index + 1,
  seasonNumber: 1,
  name: ['重逢', '旧城来信', '云海之上', '暗房微光', '失落画面', '无声放映', '回到原点', '最后一卷'][index],
  overview: `第 ${index + 1} 集的故事简介。`,
  stillPath: svgData(`第 ${index + 1} 集`, ['#354d66', '#624858', '#4b5d47'][index % 3], '#17191f', 640, 360),
  driveFiles: index === 0 ? driveFiles : [{ ...driveFiles[0], id: `scraped-file-${index + 1}`, name: `S01E${String(index + 1).padStart(2, '0')}.1080p.mp4` }]
}))

const scrapedSeries = {
  id: 'scraped-series-reference',
  parentId: 'root',
  type: 'tv',
  name: '云海纪事',
  posterUrl: svgData('云海纪事', '#493f62', '#171923', 800, 1200),
  backdropUrl: svgData('云海纪事', '#58718d', '#211a29'),
  year: '2026',
  rating: 8.6,
  genres: ['剧情', '奇幻', '悬疑'],
  productionCountries: ['中国'],
  overview: '几位年轻人在云海环绕的山城中寻找一卷失落的旧胶片，并重新理解彼此、故乡与时间。',
  driveFiles: [],
  tmdbId: 123456,
  seasons: [{ id: 1, seasonNumber: 1, name: '第 1 季', episodeCount: episodes.length, episodes }],
  credits: {
    cast: ['顾北辰', '林初夏', '沈砚', '程星河', '江晚宁', '周予安'].map((name, index) => ({ id: index + 1, name, character: ['陆川', '苏晚', '沈清', '程屿', '江宁', '周野'][index], profile_path: '' })),
    crew: []
  },
  addedAt: new Date().toISOString()
}

test('scraped media detail uses the cinematic detail layout', async ({ boxPlayer }, testInfo) => {
  const { page, pageErrors } = boxPlayer
  await page.setViewportSize({ width: 1600, height: 900 })

  const loginDialog = page.locator('.userloginmodal')
  await loginDialog.waitFor({ state: 'visible', timeout: 3_000 }).catch(() => undefined)
  if (await loginDialog.isVisible().catch(() => false)) {
    await loginDialog.getByRole('button', { name: 'Close' }).click({ force: true }).catch(() => undefined)
    await loginDialog.waitFor({ state: 'hidden', timeout: 5_000 }).catch(() => undefined)
  }
  await page.addStyleTag({ content: '.userloginmodal, .arco-modal-mask { display: none !important; pointer-events: none !important; } #xbybody { display: block !important; }' })

  if (!await page.locator('body').getAttribute('arco-theme')) await page.locator('#xbyhead2 button:has(svg.iconday)').dispatchEvent('click')
  await expect(page.locator('body')).toHaveAttribute('arco-theme', 'dark')

  await page.evaluate((item) => {
    const app = (document.querySelector('#app') as HTMLElement & { __vue_app__?: any }).__vue_app__
    const pinia = app?.config.globalProperties.$pinia
    if (!pinia) throw new Error('Pinia is unavailable in the Electron renderer')
    pinia._s.get('app')?.toggleTab('media')
    const store = pinia._s.get('mediaLibrary')
    if (!store) throw new Error('Media-library store is unavailable')
    const mediaItem = { ...item, addedAt: new Date(item.addedAt) }
    store.mediaItems = [mediaItem]
    store.recentlyAdded = [mediaItem]
    store.continueWatching = []
    store.isScanning = false
    store.updateFilters?.()
  }, scrapedSeries)

  const card = page.locator('.media-library .poster-tile:visible').filter({ hasText: scrapedSeries.name }).first()
  await expect(card).toBeVisible({ timeout: 20_000 })
  await card.dispatchEvent('click')

  const detail = page.locator('.media-detail')
  await expect(detail).toBeVisible()
  await expect(page.locator('.hero-title')).toContainText('第 1 集')
  await expect(page.locator('.hero-poster')).toBeHidden()
  await expect(page.locator('.meta-rating-logo')).toBeVisible()
  await expect(page.locator('.meta-rating-logo')).toHaveAttribute('alt', 'TMDB')
  await expect(page.locator('.version-button')).toBeVisible()
  await expect(page.locator('.play-button-label')).toContainText('播放第 1 集')
  await expect(page.locator('.episode-card')).toHaveCount(episodes.length)
  await expect(page.locator('.cast-card')).toHaveCount(scrapedSeries.credits.cast.length)
  const metadataTagsSection = page.locator('.metadata-tags-section')
  const castSection = page.locator('.cast-section')
  const mediaInfoSection = page.locator('.scraped-media-info-section')
  const detailsSection = page.locator('.details-section')
  await expect(metadataTagsSection).toBeVisible()
  await expect(metadataTagsSection.locator('.tag-group-title')).toHaveText(['类型', '制作年份', '地区'])
  await expect(detailsSection).toBeVisible()
  await expect(detailsSection.locator('.tag-group-title')).toHaveText('详细信息')
  await expect(detailsSection).toContainText('2 个文件')
  expect(await metadataTagsSection.evaluate((element) => getComputedStyle(element).flexDirection)).toBe('column')
  const sectionOrder = await page.locator('.metadata-tags-section, .cast-section, .scraped-media-info-section, .details-section').evaluateAll((elements) => elements.map(element => element.className))
  expect(sectionOrder[0]).toContain('metadata-tags-section')
  expect(sectionOrder[1]).toContain('cast-section')
  expect(sectionOrder[2]).toContain('scraped-media-info-section')
  expect(sectionOrder[3]).toContain('details-section')
  await expect(mediaInfoSection).toBeVisible()
  await expect(mediaInfoSection.locator('.detail-media-card')).toHaveCount(3)
  await expect(mediaInfoSection.locator('[data-media-kind="video"]')).toHaveCount(1)
  await expect(mediaInfoSection.locator('[data-media-kind="subtitle"]')).toHaveCount(2)
  await expect(mediaInfoSection.locator('.detail-file-name')).toHaveText('S01E01.1080p.H264.AAC.mp4')
  await expect(mediaInfoSection.locator('[data-media-kind="video"]')).toContainText('1080P · H.264')
  await expect(mediaInfoSection.locator('[data-media-kind="subtitle"]').first()).toContainText('中文')

  const heroBox = await page.locator('.hero-section').boundingBox()
  const playRowBox = await page.locator('.play-row').boundingBox()
  const playBox = await page.locator('.play-button').boundingBox()
  const versionBox = await page.locator('.version-button').boundingBox()
  expect(heroBox).not.toBeNull()
  expect(playRowBox).not.toBeNull()
  expect(playBox).not.toBeNull()
  expect(versionBox).not.toBeNull()
  expect(heroBox!.height).toBeGreaterThanOrEqual(620)
  expect((heroBox!.height - 54) / (heroBox!.width * 9 / 16)).toBeGreaterThan(0.5)
  expect(versionBox!.x).toBeGreaterThan(playBox!.x + playBox!.width)
  expect(Math.abs(versionBox!.y - playBox!.y)).toBeLessThanOrEqual(1)

  const actions = page.locator('.action-buttons .action-button')
  await expect(actions).toHaveCount(5)
  const actionBoxes = await actions.evaluateAll((elements) => elements.map((element) => {
    const box = element.getBoundingClientRect()
    return { x: box.x, width: box.width }
  }))
  for (let index = 1; index < actionBoxes.length; index += 1) {
    expect(Math.abs(actionBoxes[index].x - (actionBoxes[index - 1].x + actionBoxes[index - 1].width) - 10)).toBeLessThanOrEqual(1)
  }

  const selectedOutline = await page.locator('.episode-card.active .episode-thumbnail').evaluate((element) => getComputedStyle(element).outlineColor)
  expect(selectedOutline).toBe('rgb(255, 122, 0)')

  await page.locator('.version-button').click()
  await expect(page.getByText('S01E01.2160p.HEVC.DV.mp4')).toBeVisible()
  await page.getByText('S01E01.2160p.HEVC.DV.mp4').click()
  await expect(page.locator('.detail-version-popup')).toBeHidden()
  await expect(page.getByText('S01E01.1080p.H264.AAC.mp4')).toBeHidden()
  await expect(mediaInfoSection.locator('.detail-file-name')).toHaveText('S01E01.2160p.HEVC.DV.mp4')
  await expect(mediaInfoSection.locator('.detail-media-card')).toHaveCount(2)
  await expect(mediaInfoSection.locator('[data-media-kind="video"]')).toContainText('2160P · HEVC · Dolby Vision')
  await expect(mediaInfoSection.locator('[data-media-kind="subtitle"]')).toContainText('S01E01.2160p.zh-CN.ass')

  await page.screenshot({ path: testInfo.outputPath('scraped-media-detail-reference-layout.png'), fullPage: false })
  await mediaInfoSection.scrollIntoViewIfNeeded()
  await page.screenshot({ path: testInfo.outputPath('scraped-media-detail-media-cards.png'), fullPage: false })
  await detailsSection.scrollIntoViewIfNeeded()
  await page.screenshot({ path: testInfo.outputPath('scraped-media-detail-bottom-info.png'), fullPage: false })
  expect(pageErrors).toEqual([])
})
