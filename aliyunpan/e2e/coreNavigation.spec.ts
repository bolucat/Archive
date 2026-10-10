import { expect, test } from './fixtures/boxPlayer'

test('top navigation opens every core workspace', async ({ boxPlayer }) => {
  const { page, pageErrors, consoleErrors } = boxPlayer
  await page.setViewportSize({ width: 1600, height: 900 })
  const workspaces = ['search', 'ai-workspace', 'media', 'down', 'share', 'rss']

  for (const workspace of workspaces) {
    await test.step(workspace, async () => {
      const navItem = page.locator('#xbyhead2').getByTestId(`top-nav-${workspace}`)
      await navItem.click()
      await expect(navItem).toHaveClass(/arco-menu-selected/)
      await expect(page.locator('#xbybody')).toBeVisible()
    })
  }

  await test.step('聚合媒体库中的音乐与书籍', async () => {
    await page.locator('#xbyhead2').getByTestId('top-nav-media').click()
    const library = page.getByTestId('unified-media-library')
    await expect(library).toBeVisible()
    for (const section of ['music', 'book']) {
      await library.getByTestId(`unified-nav-${section}`).click()
      await expect(library.getByTestId(`unified-nav-${section}`)).toHaveClass(/selected/)
      await expect(library.getByTestId(`unified-section-${section}`)).toBeVisible()
    }
  })

  await test.step('设置', async () => {
    await page.getByTestId('open-settings').click()
    await expect(page.locator('#SettingUI')).toBeVisible()
  })

  expect(pageErrors).toEqual([])
  expect(consoleErrors).toEqual([])
})
