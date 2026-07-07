import { quarkRefreshToken, quarkListDir, quarkWalk, quarkSearch, quarkGetFile, quarkRenameBatch, quarkMoveBatch, quarkMkdir, quarkTrash } from './quark.mjs'

export function createQuarkProvider() {
  return {
    id: 'quark',
    displayName: '夸克网盘',
    capabilities: {
      batchRename: true,
      recursiveWalk: true,
      serverSideSearch: true,
      trash: true,
      permanentDelete: false,
      share: true,
      pathAddressable: false,
      fileIdAddressable: true,
      mkdir: true,
      move: true,
      uploadFile: false,
    },
    auth: {
      async login() {
        const err = new Error('Interactive login not supported. Import a Quark token exported from BoxPlayer.')
        err.code = 'ERR_PROVIDER_OPERATION_UNIMPLEMENTED'
        throw err
      },
      async refresh(token) { return quarkRefreshToken(token) },
      async listAccounts() { return [] },
    },
    files: {
      async list({ token, parentFileId = 'quark_root' }) { return quarkListDir(token, parentFileId) },
      async *walk({ token, parentFileId = 'quark_root', maxDepth = 10 }) { yield* quarkWalk(token, parentFileId, maxDepth) },
      async get({ token, fileId }) { return quarkGetFile(token, fileId) },
      async rename({ token, fileId, newName }) { return (await quarkRenameBatch(token, [{ fileId, newName }]))[0] },
      async renameBatch({ token, renames }) { return quarkRenameBatch(token, renames) },
      async search({ token, name, limit }) { return quarkSearch(token, name, { limit }) },
      async moveBatch({ token, moves }) { return quarkMoveBatch(token, moves) },
      async mkdir({ token, parentId = 'quark_root', name }) { return quarkMkdir(token, parentId, name) },
      async trash({ token, items }) { return quarkTrash(token, items.map((i) => i.fileId)) },
    },
  }
}
