import { describe, expect, it } from 'vitest'
import { readFileSync } from 'fs'
import { resolve } from 'path'

const root = resolve(__dirname, '../../..')
const readSource = (file: string) => readFileSync(resolve(root, file), 'utf8')

describe('video library scan controls', () => {
  it('uses logged-in cloud accounts as video scan sources before falling back to existing folders', () => {
    const source = readSource('src/components/MediaLibraryNav.vue')

    expect(source).toContain("import UserDAL from '../user/userdal'")
    expect(source).toContain("import type { ITokenInfo } from '../user/userstore'")
    expect(source).toContain('const scanAccounts = ref<ITokenInfo[]>([])')
    expect(source).toContain('const scanSourceTargets = computed<ScanSourceTarget[]>')
    expect(source).toContain('for (const token of scanAccounts.value)')
    expect(source).toContain('tokenRootScanFolders(token)')
    expect(source).toContain('const users = await UserDAL.GetUserListFromDB().catch(() => [])')
    expect(source).toContain('scanAccounts.value = users.filter((u) => !!u?.user_id && !!u?.access_token)')
    expect(source).toContain("value: `account:${token.user_id}:${item.driveId}:root`")
    expect(source).toContain("cloud123: { driveId: 'cloud123', fileId: '0' }")
    expect(source).toContain("'115': { driveId: 'drive115', fileId: '0' }")
    expect(source).toContain("baidu: { driveId: 'baidu', fileId: '/', path: '/' }")
    expect(source).toContain("pikpak: { driveId: 'pikpak', fileId: 'pikpak_root' }")
    expect(source).toContain("quark: { driveId: 'quark', fileId: '0' }")
    expect(source).toContain("'139': { driveId: 'cloud139', fileId: 'cloud139_root' }")
    expect(source).toContain("'189': { driveId: 'cloud189', fileId: 'cloud189_root' }")
    expect(source).toContain('mediaFolderToAliModel(folder)')
    expect(source).toContain('const targets = selected.size ? scanSourceTargets.value.filter')
    expect(source).toContain('await mediaScanner.scanFolder(target.folder, target.driveServerId, { incremental: true })')
  })

  it('keeps the video scanner wired to the same major cloud providers as the scan dropdown', () => {
    const scanner = readSource('src/utils/mediaScanner.ts')

    expect(scanner).toContain("from '../quark/dirfilelist'")
    expect(scanner).toContain("from '../cloud139/dirfilelist'")
    expect(scanner).toContain("from '../cloud189/dirfilelist'")
    expect(scanner).toContain("driveId === 'quark'")
    expect(scanner).toContain("driveId === 'cloud139'")
    expect(scanner).toContain("driveId === 'cloud189'")
    expect(scanner).toContain('apiQuarkFileList')
    expect(scanner).toContain('apiCloud139FileList')
    expect(scanner).toContain('apiCloud189FileList')
  })

  it('keeps local import and WebDAV actions visually aligned with the scan panel buttons', () => {
    const panel = readSource('src/components/LibraryScanPanel.vue')

    expect(panel).toContain('#xbybody .media-library-nav .library-scan-import-btn')
    expect(panel).toContain('border-radius: 999px !important')
    expect(panel).toContain('background: rgba(255, 255, 255, .055) !important')
    expect(panel).toContain('color: rgba(255, 255, 255, .72) !important')
    expect(panel).toContain('font-weight: 500 !important')
    expect(panel).toContain('box-shadow: none !important')
    expect(panel).toContain('.media-library-nav .library-scan-import-btn:hover')
  })
})
