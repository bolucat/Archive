import { cloud189RefreshToken, cloud189ListDir, cloud189Walk, cloud189GetFile, cloud189RenameBatch, cloud189MoveBatch, cloud189Mkdir, cloud189Trash } from './cloud189.mjs'

export function createCloud189Provider() {
  return {
    id: '189',
    displayName: '天翼云盘',
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
        const err = new Error('Interactive login not supported. Import a 189 token exported from BoxPlayer.')
        err.code = 'ERR_PROVIDER_OPERATION_UNIMPLEMENTED'
        throw err
      },
      async refresh(token) { return cloud189RefreshToken(token) },
      async listAccounts() { return [] },
    },
    files: {
      async list({ token, parentFileId = 'cloud189_root' }) { return cloud189ListDir(token, parentFileId) },
      async *walk({ token, parentFileId = 'cloud189_root', maxDepth = 10 }) { yield* cloud189Walk(token, parentFileId, maxDepth) },
      async get({ token, fileId }) { return cloud189GetFile(token, fileId) },
      async rename({ token, fileId, newName }) { return (await cloud189RenameBatch(token, [{ fileId, newName }]))[0] },
      async renameBatch({ token, renames }) { return cloud189RenameBatch(token, renames) },
      async moveBatch({ token, moves }) { return cloud189MoveBatch(token, moves) },
      async mkdir({ token, parentId = 'cloud189_root', name }) { return cloud189Mkdir(token, parentId, name) },
      async trash({ token, items }) { return cloud189Trash(token, items.map((i) => i.fileId)) },
    },
  }
}
