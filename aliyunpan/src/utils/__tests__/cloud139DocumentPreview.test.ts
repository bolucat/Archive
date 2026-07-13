import { describe, expect, it } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'

const root = path.resolve(__dirname, '../../..')

describe('cloud139 document preview routing', () => {
  it('routes cloud139 PDFs through raw-url PDF preview instead of Aliyun office preview', () => {
    const source = fs.readFileSync(path.join(root, 'src/utils/openfile.ts'), 'utf8')

    expect(source).toContain("const PDF_PREVIEW_DRIVES = new Set(['cloud123', 'drive115', 'baidu', 'pikpak', 'dropbox', 'onedrive', 'box', 'guangya', 'cloud139', 'cloud189'])")
    expect(source).toContain("if ((file.ext || '').toLowerCase() === 'pdf' && PDF_PREVIEW_DRIVES.has(file.drive_id || ''))")
  })

  it('does not call Aliyun office preview for cloud139 drives', () => {
    const source = fs.readFileSync(path.join(root, 'src/aliapi/file.ts'), 'utf8')

    expect(source).toContain("isCloud139User(user_id) || drive_id === 'cloud139'")
    expect(source).toContain("const url = 'v2/file/get_office_preview_url'")
  })

  it('does not call Aliyun video preview for cloud139 drives', () => {
    const source = fs.readFileSync(path.join(root, 'src/aliapi/file.ts'), 'utf8')

    expect(source).toContain("if (isCloud139User(user_id) || drive_id === 'cloud139') {\n      return '暂无转码信息'\n    }")
    expect(source).toContain("url = 'adrive/v1.0/openFile/getVideoPreviewPlayInfo'")
  })

  it('refreshes inline document proxy urls with origin quality', () => {
    const source = fs.readFileSync(path.join(root, 'src/utils/proxyhelper.ts'), 'utf8')

    expect(source).toContain("const refreshQuality = content_disposition === 'inline' ? 'Origin' : selectQuality")
    expect(source).toContain("getRawUrl(user_id, drive_id, file_id, encType, '', weifa, 'other', refreshQuality)")
  })

  it('does not call Aliyun audio preview/path/folder-size APIs for cloud139 drives', () => {
    const source = fs.readFileSync(path.join(root, 'src/aliapi/file.ts'), 'utf8')

    expect(source).toContain("isCloud139User(user_id) || drive_id === 'cloud139' || isCloud189User(user_id) || drive_id === 'cloud189'")
    expect(source).toContain("const url = 'v2/file/get_audio_play_info'")
    expect(source).toContain("const url = 'adrive/v1/file/get_path'")
    expect(source).toContain("const url = 'adrive/v1/file/get_folder_size_info'")
  })

  it('routes third-party image preview through the proxy instead of Aliyun download urls', () => {
    const source = fs.readFileSync(path.join(root, 'src/layout/PageImage.vue'), 'utf8')

    expect(source).toContain("const thirdPartyImageDrives = new Set(['cloud123', 'drive115', 'baidu', 'pikpak', 'quark', 'cloud139', 'cloud189', 'dropbox', 'onedrive', 'box', 'guangya'])")
  })
})
