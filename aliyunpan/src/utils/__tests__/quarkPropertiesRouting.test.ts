import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

describe('Quark properties routing', () => {
  it('uses Quark detail and local tree paths instead of Aliyun property APIs', () => {
    const modal = readFileSync(resolve(process.cwd(), 'src/pan/topbtns/ShuXingModal.vue'), 'utf8')
    const fileApi = readFileSync(resolve(process.cwd(), 'src/aliapi/file.ts'), 'utf8')

    expect(modal).toContain("const isQuark = isQuarkUser(pantreeStore.user_id) || drive_id === 'quark'")
    expect(modal).toContain('await apiQuarkFileDetail(pantreeStore.user_id, file_id)')
    expect(modal).toContain('!isPikPak && !isQuark && !isDropbox')

    const pathString = fileApi.slice(fileApi.indexOf('static async ApiFileGetPathString'), fileApi.indexOf('static async ApiFileGetFolderSize'))
    const folderSize = fileApi.slice(fileApi.indexOf('static async ApiFileGetFolderSize'), fileApi.indexOf('static async ApiFileGetVideoPreviewPlayInfo'))
    expect(pathString).toContain("isQuarkUser(user_id) || drive_id === 'quark'")
    expect(folderSize).toContain("isQuarkUser(user_id) || drive_id === 'quark'")
  })
})
