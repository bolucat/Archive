import { dropboxRefreshToken, dropboxListDir, dropboxWalk, dropboxRenameBatch, dropboxSearch, dropboxGetFile, dropboxMove, dropboxMkdir, dropboxDelete, dropboxUploadFile, dropboxDownloadFile } from './dropbox.mjs'

export function createDropboxProvider() {
  return {
    id: 'dropbox',
    displayName: 'Dropbox',
    capabilities: {
      batchRename: true,
      recursiveWalk: true,
      serverSideSearch: true,
      trash: false,
      permanentDelete: true,
      share: true,
      pathAddressable: true,
      fileIdAddressable: true,
      mkdir: true,
      move: true,
      uploadFile: true,
      downloadFile: true,
    },
    auth: {
      async login() {
        const err = new Error('Interactive login not supported. Set token in ~/.clouddrive-cli/tokens.json.')
        err.code = 'ERR_PROVIDER_OPERATION_UNIMPLEMENTED'
        throw err
      },
      async refresh(token) { return dropboxRefreshToken(token) },
      async listAccounts() { return [] },
    },
    files: {
      async list({ token, parentFileId = '' }) { return dropboxListDir(token, parentFileId) },
      async *walk({ token, parentFileId = '', maxDepth = 10 }) { yield* dropboxWalk(token, parentFileId, maxDepth) },
      async get({ token, fileId }) { return dropboxGetFile(token, fileId) },
      async rename({ token, fileId, newName }) { return (await dropboxRenameBatch(token, [{ fileId, newName }]))[0] },
      async renameBatch({ token, renames }) { return dropboxRenameBatch(token, renames) },
      async search({ token, name, limit }) { return dropboxSearch(token, name, { limit }) },
      async moveBatch({ token, moves }) { return dropboxMove(token, moves) },
      async mkdir({ token, parentPath = '', name }) { return dropboxMkdir(token, parentPath, name) },
      // Dropbox has no trash: use permanent delete
      async trash({ token, items }) { return dropboxDelete(token, items) },
      async uploadFile({ token, parentId, parentPath, localPath, name, size, conflict }) {
        return dropboxUploadFile(token, { parentId: parentPath || parentId, localPath, name, size, conflict })
      },
      async downloadFile({ token, fileId, outputPath }) {
        return dropboxDownloadFile(token, fileId, outputPath)
      },
    },
  }
}
