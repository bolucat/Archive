import { cloud139RefreshToken, cloud139ListDir, cloud139Walk, cloud139GetFile, cloud139RenameBatch, cloud139MoveBatch, cloud139Mkdir, cloud139Trash } from './cloud139.mjs'

export function createCloud139Provider() {
  return {
    id: '139',
    displayName: '139云盘',
    capabilities: {
      batchRename: true,
      recursiveWalk: true,
      serverSideSearch: false,
      trash: true,
      permanentDelete: false,
      share: false,
      pathAddressable: false,
      fileIdAddressable: true,
      mkdir: true,
      move: true,
      uploadFile: false,
    },
    auth: {
      async login() {
        const err = new Error('Interactive login not supported. Import a 139 token exported from BoxPlayer.')
        err.code = 'ERR_PROVIDER_OPERATION_UNIMPLEMENTED'
        throw err
      },
      async refresh(token) { return cloud139RefreshToken(token) },
      async listAccounts() { return [] },
    },
    files: {
      async list({ token, parentFileId = 'cloud139_root' }) { return cloud139ListDir(token, parentFileId) },
      async *walk({ token, parentFileId = 'cloud139_root', maxDepth = 10 }) { yield* cloud139Walk(token, parentFileId, maxDepth) },
      async get({ token, fileId }) { return cloud139GetFile(token, fileId) },
      async rename({ token, fileId, newName }) { return (await cloud139RenameBatch(token, [{ fileId, newName }]))[0] },
      async renameBatch({ token, renames }) { return cloud139RenameBatch(token, renames) },
      async moveBatch({ token, moves }) { return cloud139MoveBatch(token, moves) },
      async mkdir({ token, parentId = 'cloud139_root', name }) { return cloud139Mkdir(token, parentId, name) },
      async trash({ token, items }) { return cloud139Trash(token, items.map((i) => i.fileId)) },
    },
  }
}
