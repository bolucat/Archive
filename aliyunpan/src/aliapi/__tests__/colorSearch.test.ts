import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { buildAliColorSearchDriveIds } from '../colorSearch'

describe('Aliyun color-label search', () => {
  it('searches backup and resource drives without duplicates or empty ids', () => {
    expect(buildAliColorSearchDriveIds('backup-drive', 'resource-drive')).toEqual(['backup-drive', 'resource-drive'])
    expect(buildAliColorSearchDriveIds('same-drive', 'same-drive')).toEqual(['same-drive'])
    expect(buildAliColorSearchDriveIds('', 'resource-drive')).toEqual(['resource-drive'])
  })

  it('routes color labels through the Aliyun multi-drive search endpoint', () => {
    const source = readFileSync(new URL('../dirfilelist.ts', import.meta.url), 'utf8')
    const pageSearch = source.slice(source.indexOf('static async _ApiSearchFileListOnePage'), source.indexOf('static async _ApiSearchFileListCount'))
    expect(pageSearch).toContain("url = 'adrive/v3/file/search'")
    expect(pageSearch).toContain('postData.drive_id_list = buildAliColorSearchDriveIds')
    expect(pageSearch).toContain('delete postData.drive_id')
  })
})
