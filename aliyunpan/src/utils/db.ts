import Dexie from 'dexie'
import { ITokenInfo } from '../user/userstore'
import { IOtherShareLinkModel } from '../share/share/OtherShareStore'
import { IMusicTrack } from '../types/music'
import { IBookItem } from '../types/book'
import { IBookNote } from '../types/bookNote'
import { IBookBookmark } from '../types/bookBookmark'
import type { MediaLibraryFolder, MediaLibraryItem } from '../types/media'
import type { AIConversation, AIMessage, BookIndexMeta } from '../services/ai/types'
import type { TextChunk } from './bookAI'

type AIConversationRecord = AIConversation
type AIMessageRecord = AIMessage

class XBYDB3 extends Dexie {
  iobject: Dexie.Table<object, string>
  istring: Dexie.Table<string, string>
  inumber: Dexie.Table<number, string>
  ibool: Dexie.Table<boolean, string>
  icache: Dexie.Table<Blob, string>

  itoken: Dexie.Table<ITokenInfo, string>
  iothershare: Dexie.Table<IOtherShareLinkModel, string>
  imusic_track: Dexie.Table<IMusicTrack, string>
  ibook_item: Dexie.Table<IBookItem, string>
  ibook_note: Dexie.Table<IBookNote, string>
  ibook_bookmark: Dexie.Table<IBookBookmark, string>
  ibook_ai_chunk: Dexie.Table<TextChunk, string>
  ibook_ai_meta: Dexie.Table<BookIndexMeta, string>
  ibook_ai_bm25: Dexie.Table<{ id: string; bookId: string; sourceHash: string; settingsHash: string; updatedAt: number }, string>
  ibook_ai_conversation: Dexie.Table<AIConversationRecord, string>
  ibook_ai_message: Dexie.Table<AIMessageRecord, string>
  imedia_item: Dexie.Table<MediaLibraryItem, string>
  imedia_folder: Dexie.Table<MediaLibraryFolder, string>
  imedia_file: Dexie.Table<{ id: string; mediaId: string; folderId?: string }, string>

  constructor() {
    super('XBY3Database')

    this.version(10)
      .stores({
        iobject: '',
        istring: '',
        inumber: '',
        ibool: '',
        icache: '',

        itoken: 'user_id',
        iothershare: 'share_id'
      })
      .upgrade((tx: any) => {
        console.log('upgrade', tx)
      })

    this.version(11)
      .stores({
        iobject: '',
        istring: '',
        inumber: '',
        ibool: '',
        icache: '',

        itoken: 'user_id',
        iothershare: 'share_id',
        imusic_track: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, artist, album'
      })
      .upgrade((tx: any) => {
        console.log('upgrade to v11 (music_track)', tx)
      })

    this.version(12)
      .stores({
        iobject: '',
        istring: '',
        inumber: '',
        ibool: '',
        icache: '',

        itoken: 'user_id',
        iothershare: 'share_id',
        imusic_track: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, artist, album',
        ibook_item: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, author, ext'
      })
      .upgrade((tx: any) => {
        console.log('upgrade to v12 (book_item)', tx)
      })

    this.version(13)
      .stores({
        iobject: '',
        istring: '',
        inumber: '',
        ibool: '',
        icache: '',

        itoken: 'user_id',
        iothershare: 'share_id',
        imusic_track: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, artist, album',
        ibook_item: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, author, ext',
        ibook_note: '&id, book_id, [user_id+drive_id], user_id, drive_id, file_id, kind, created_at, updated_at'
      })
      .upgrade((tx: any) => {
        console.log('upgrade to v13 (book_note)', tx)
      })

    this.version(14)
      .stores({
        iobject: '',
        istring: '',
        inumber: '',
        ibool: '',
        icache: '',

        itoken: 'user_id',
        iothershare: 'share_id',
        imusic_track: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, artist, album',
        ibook_item: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, author, ext',
        ibook_note: '&id, book_id, [user_id+drive_id], user_id, drive_id, file_id, kind, created_at, updated_at',
        ibook_bookmark: '&id, book_id, [user_id+drive_id], user_id, drive_id, file_id, percentage, created_at, updated_at'
      })
      .upgrade((tx: any) => {
        console.log('upgrade to v14 (book_bookmark)', tx)
      })

    this.version(15)
      .stores({
        iobject: '',
        istring: '',
        inumber: '',
        ibool: '',
        icache: '',

        itoken: 'user_id',
        iothershare: 'share_id',
        imusic_track: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, artist, album',
        ibook_item: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, author, ext',
        ibook_note: '&id, book_id, [user_id+drive_id], user_id, drive_id, file_id, kind, created_at, updated_at',
        ibook_bookmark: '&id, book_id, [user_id+drive_id], user_id, drive_id, file_id, percentage, created_at, updated_at',
        ibook_ai_chunk: '&id, bookId, [bookId+sourceHash], [bookId+settingsHash], sourceHash, settingsHash, sectionIndex, pageNumber',
        ibook_ai_meta: '&id, bookId, [bookId+sourceHash], [bookId+settingsHash], sourceHash, settingsHash, lastUpdated',
        ibook_ai_bm25: '&id, bookId, [bookId+sourceHash], [bookId+settingsHash], sourceHash, settingsHash, updatedAt',
        ibook_ai_conversation: '&id, bookId, [bookId+mode], updatedAt',
        ibook_ai_message: '&id, conversationId, createdAt'
      })

    this.version(16)
      .stores({
        iobject: '', istring: '', inumber: '', ibool: '', icache: '', itoken: 'user_id', iothershare: 'share_id',
        imusic_track: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, artist, album',
        ibook_item: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, author, ext',
        ibook_note: '&id, book_id, [user_id+drive_id], user_id, drive_id, file_id, kind, created_at, updated_at',
        ibook_bookmark: '&id, book_id, [user_id+drive_id], user_id, drive_id, file_id, percentage, created_at, updated_at',
        ibook_ai_chunk: '&id, bookId, [bookId+sourceHash], [bookId+settingsHash], sourceHash, settingsHash, sectionIndex, pageNumber',
        ibook_ai_meta: '&id, bookId, [bookId+sourceHash], [bookId+settingsHash], sourceHash, settingsHash, lastUpdated',
        ibook_ai_bm25: '&id, bookId, [bookId+sourceHash], [bookId+settingsHash], sourceHash, settingsHash, updatedAt',
        ibook_ai_conversation: '&id, bookId, [bookId+mode], updatedAt', ibook_ai_message: '&id, conversationId, createdAt',
        imedia_item: '&id, folderId, type, addedAt, tmdbId', imedia_folder: '&id, [userId+driveId], fileId, scanDate'
      })

    this.version(17)
      .stores({
        iobject: '', istring: '', inumber: '', ibool: '', icache: '', itoken: 'user_id', iothershare: 'share_id',
        imusic_track: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, artist, album',
        ibook_item: '&id, [user_id+drive_id], user_id, drive_id, parent_file_id, scanned_at, updated_at, author, ext',
        ibook_note: '&id, book_id, [user_id+drive_id], user_id, drive_id, file_id, kind, created_at, updated_at', ibook_bookmark: '&id, book_id, [user_id+drive_id], user_id, drive_id, file_id, percentage, created_at, updated_at',
        ibook_ai_chunk: '&id, bookId, [bookId+sourceHash], [bookId+settingsHash], sourceHash, settingsHash, sectionIndex, pageNumber', ibook_ai_meta: '&id, bookId, [bookId+sourceHash], [bookId+settingsHash], sourceHash, settingsHash, lastUpdated', ibook_ai_bm25: '&id, bookId, [bookId+sourceHash], [bookId+settingsHash], sourceHash, settingsHash, updatedAt', ibook_ai_conversation: '&id, bookId, [bookId+mode], updatedAt', ibook_ai_message: '&id, conversationId, createdAt',
        imedia_item: '&id, folderId, type, addedAt, tmdbId', imedia_folder: '&id, [userId+driveId], fileId, scanDate', imedia_file: '&id, mediaId, folderId'
      })
      .upgrade((tx: any) => {
        console.log('upgrade to v15 (book_ai)', tx)
      })

    this.iobject = this.table('iobject')
    this.istring = this.table('istring')
    this.inumber = this.table('inumber')
    this.ibool = this.table('ibool')
    this.icache = this.table('icache')

    this.itoken = this.table('itoken')
    this.iothershare = this.table('iothershare')
    this.imusic_track = this.table('imusic_track')
    this.ibook_item = this.table('ibook_item')
    this.ibook_note = this.table('ibook_note')
    this.ibook_bookmark = this.table('ibook_bookmark')
    this.ibook_ai_chunk = this.table('ibook_ai_chunk')
    this.ibook_ai_meta = this.table('ibook_ai_meta')
    this.ibook_ai_bm25 = this.table('ibook_ai_bm25')
    this.ibook_ai_conversation = this.table('ibook_ai_conversation')
    this.ibook_ai_message = this.table('ibook_ai_message')
    this.imedia_item = this.table('imedia_item')
    this.imedia_folder = this.table('imedia_folder')
    this.imedia_file = this.table('imedia_file')
  }

  async getValueString(key: string): Promise<string> {
    if (!this.isOpen()) await this.open().catch(() => {})
    const val = await this.istring.get(key)
    if (val) return val
    else return ''
  }

  async saveValueString(key: string, value: string): Promise<string> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.istring.put(value || '', key)
  }

  async saveValueStringBatch(keys: string[], values: string[]): Promise<string> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.istring.bulkPut(values, keys)
  }

  async getValueNumber(key: string): Promise<number> {
    if (!this.isOpen()) await this.open().catch(() => {})
    const val = await this.inumber.get(key)
    if (val) return val
    return 0
  }

  async saveValueNumber(key: string, value: number): Promise<string> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.inumber.put(value, key)
  }

  async getValueBool(key: string): Promise<boolean> {
    if (!this.isOpen()) await this.open().catch(() => {})
    const val = await this.ibool.get(key)
    if (val) return true
    return false
  }

  async saveValueBool(key: string, value: boolean): Promise<string> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibool.put(value || false, key)
  }

  async getValueObject(key: string): Promise<object | undefined> {
    if (!this.isOpen()) await this.open().catch(() => {})
    const val = await this.iobject.get(key)
    if (val) return val
    else return undefined
  }

  async saveValueObject(key: string, value: object): Promise<string | void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.iobject.put(value, key).catch(() => {})
  }

  async saveValueObjectBatch(keys: string[], values: object[]): Promise<string> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.iobject.bulkPut(values, keys)
  }

  async deleteValueObject(key: string): Promise<void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.iobject.delete(key)
  }

  async saveMediaLibrary(items: MediaLibraryItem[], folders: MediaLibraryFolder[]): Promise<void> {
    if (!this.isOpen()) await this.open()
    const files = items.flatMap((item) => {
      const all = [...(item.driveFiles || []), ...(item.seasons || []).flatMap(season => (season.episodes || []).flatMap(episode => episode.driveFiles || []))]
      return all.filter(file => !!file.id).map(file => ({ id: [file.driveServerId, file.userId, file.driveId, file.id].map(value => encodeURIComponent(String(value || ''))).join(':'), mediaId: item.id, folderId: item.folderId }))
    })
    await this.transaction('rw', this.imedia_item, this.imedia_folder, this.imedia_file, async () => {
      await this.imedia_item.clear()
      await this.imedia_folder.clear()
      await this.imedia_file.clear()
      await this.imedia_item.bulkPut(items)
      await this.imedia_folder.bulkPut(folders)
      await this.imedia_file.bulkPut(files)
    })
  }

  async getMediaLibrary(): Promise<{ items: MediaLibraryItem[]; folders: MediaLibraryFolder[] }> {
    if (!this.isOpen()) await this.open()
    const [items, folders] = await Promise.all([this.imedia_item.toArray(), this.imedia_folder.toArray()])
    return { items, folders }
  }

  async getMediaLibraryPage(options: { offset?: number; limit?: number; folderId?: string; type?: MediaLibraryItem['type']; predicate?: (item: MediaLibraryItem) => boolean } = {}): Promise<MediaLibraryItem[]> {
    if (!this.isOpen()) await this.open()
    const { offset = 0, limit = 100, folderId, type, predicate } = options
    let collection: Dexie.Collection<MediaLibraryItem, string>
    if (folderId) collection = this.imedia_item.where('folderId').equals(folderId)
    else if (type) collection = this.imedia_item.where('type').equals(type)
    else collection = this.imedia_item.orderBy('addedAt')
    const filtered = type && folderId ? collection.filter(item => item.type === type) : collection
    const matched = predicate ? filtered.filter(predicate) : filtered
    return matched.reverse().offset(offset).limit(limit).toArray()
  }

  async getMediaLibraryItemsByIds(ids: string[]): Promise<MediaLibraryItem[]> {
    if (!ids.length) return []
    if (!this.isOpen()) await this.open()
    return this.imedia_item.bulkGet(ids).then(items => items.filter((item): item is MediaLibraryItem => !!item))
  }

  async countMediaLibraryItems(options: { folderId?: string; type?: MediaLibraryItem['type']; predicate?: (item: MediaLibraryItem) => boolean } = {}): Promise<number> {
    if (!this.isOpen()) await this.open()
    let collection: Dexie.Collection<MediaLibraryItem, string>
    if (options.folderId) collection = this.imedia_item.where('folderId').equals(options.folderId)
    else if (options.type) collection = this.imedia_item.where('type').equals(options.type)
    else collection = this.imedia_item.toCollection()
    const matched = options.type && options.folderId ? collection.filter(item => item.type === options.type) : collection
    return options.predicate ? matched.filter(options.predicate).count() : matched.count()
  }

  async getMediaLibraryFolders(): Promise<MediaLibraryFolder[]> {
    if (!this.isOpen()) await this.open()
    return this.imedia_folder.toArray()
  }

  async getIndexedMediaFileIds(ids: string[]): Promise<Set<string>> {
    if (!ids.length) return new Set()
    if (!this.isOpen()) await this.open()
    const records = await this.imedia_file.bulkGet(ids)
    return new Set(records.filter((record): record is { id: string; mediaId: string; folderId?: string } => !!record).map(record => record.id))
  }

  async upsertMediaLibraryItems(items: MediaLibraryItem[]): Promise<void> {
    if (!items.length) return
    if (!this.isOpen()) await this.open()
    const existingItems = await this.imedia_item.bulkGet(items.map(item => item.id))
    const mergedItems = items.map((item, index) => {
      const existing = existingItems[index]
      if (!item.collectionId || !existing?.collectionId) return item

      const driveFileKey = (file: MediaLibraryItem['driveFiles'][number]) => [file.driveServerId, file.userId, file.driveId, file.id].join(':')
      const mergeDriveFiles = (left: MediaLibraryItem['driveFiles'], right: MediaLibraryItem['driveFiles']) => {
        const files = new Map(left.map(file => [driveFileKey(file), file]))
        right.forEach(file => files.set(driveFileKey(file), file))
        return Array.from(files.values())
      }
      const movies = new Map((existing.collectionMovies || []).map(movie => [movie.tmdbId || movie.id, movie]))
      for (const movie of item.collectionMovies || []) {
        const key = movie.tmdbId || movie.id
        const previous = movies.get(key)
        movies.set(key, previous ? { ...previous, ...movie, driveFiles: mergeDriveFiles(previous.driveFiles || [], movie.driveFiles || []) } : movie)
      }
      return {
        ...existing,
        ...item,
        driveFiles: mergeDriveFiles(existing.driveFiles || [], item.driveFiles || []),
        collectionMovies: Array.from(movies.values()).sort((a, b) => Number(a.year || 0) - Number(b.year || 0))
      }
    })
    const files = mergedItems.flatMap((item) => [...(item.driveFiles || []), ...(item.seasons || []).flatMap(season => (season.episodes || []).flatMap(episode => episode.driveFiles || []))]
      .filter(file => !!file.id).map(file => ({ id: [file.driveServerId, file.userId, file.driveId, file.id].map(value => encodeURIComponent(String(value || ''))).join(':'), mediaId: item.id, folderId: item.folderId })))
    await this.transaction('rw', this.imedia_item, this.imedia_file, async () => {
      await this.imedia_item.bulkPut(mergedItems)
      await this.imedia_file.where('mediaId').anyOf(mergedItems.map(item => item.id)).delete()
      await this.imedia_file.bulkPut(files)
    })
  }

  async deleteMediaLibraryItems(ids: string[]): Promise<void> {
    if (!ids.length) return
    if (!this.isOpen()) await this.open()
    await this.transaction('rw', this.imedia_item, this.imedia_file, async () => {
      await this.imedia_item.bulkDelete(ids)
      await this.imedia_file.where('mediaId').anyOf(ids).delete()
    })
  }

  async upsertMediaLibraryFolders(folders: MediaLibraryFolder[]): Promise<void> {
    if (!folders.length) return
    if (!this.isOpen()) await this.open()
    await this.imedia_folder.bulkPut(folders)
  }

  async deleteMediaLibraryFolders(ids: string[]): Promise<void> {
    if (!ids.length) return
    if (!this.isOpen()) await this.open()
    await this.imedia_folder.bulkDelete(ids)
  }

  async clearMediaLibrary(): Promise<void> {
    if (!this.isOpen()) await this.open()
    await this.transaction('rw', this.imedia_item, this.imedia_folder, this.imedia_file, async () => {
      await this.imedia_item.clear()
      await this.imedia_folder.clear()
      await this.imedia_file.clear()
    })
  }

  async getUser(user_id: string): Promise<ITokenInfo | undefined> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.transaction('r', this.itoken, () => {
      return this.itoken.get(user_id)
    })
  }

  async getUserAll(): Promise<ITokenInfo[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    const list = await this.transaction('r', this.itoken, () => {
      return this.itoken.toArray()
    })
    return list.sort((a: ITokenInfo, b: ITokenInfo) => b.used_size - a.used_size)
  }

  async deleteUser(user_id: string): Promise<void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.itoken.delete(user_id)
  }

  async saveUser(token: ITokenInfo): Promise<string | void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.itoken.put(token, token.user_id).catch(() => {})
  }

  async saveUserBatch(tokens: ITokenInfo[]): Promise<boolean | string> {
    if (tokens.length == 0) return false
    if (!this.isOpen()) await this.open().catch()
    return this.itoken.bulkPut(tokens).catch()
  }

  async getCache(key: string): Promise<Blob | undefined> {
    if (!this.isOpen()) await this.open().catch(() => {})
    const val = await this.icache.get(key)
    return val
  }

  async saveCache(key: string, data: Blob) {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.icache.put(data, key)
  }

  async getOtherShare(share_id: string): Promise<IOtherShareLinkModel | undefined> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.iothershare.get(share_id)
  }

  async getOtherShareAll(): Promise<IOtherShareLinkModel[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    const list = await this.iothershare.toArray()
    return list.sort((a: IOtherShareLinkModel, b: IOtherShareLinkModel) => b.saved_time - a.saved_time)
  }

  async deleteOtherShareBatch(share_id_list: string[]): Promise<void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.iothershare.bulkDelete(share_id_list)
  }

  async saveOtherShare(share: IOtherShareLinkModel): Promise<string | void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.iothershare.put(share, share.share_id).catch(() => {})
  }

  async saveMusicTracks(tracks: IMusicTrack[]): Promise<string | void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    if (!tracks.length) return
    return this.imusic_track.bulkPut(tracks).catch(() => {})
  }

  async getMusicTrackById(id: string): Promise<IMusicTrack | undefined> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.imusic_track.get(id)
  }

  async getMusicTracksByIds(ids: string[]): Promise<IMusicTrack[]> {
    if (!ids.length) return []
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.imusic_track.bulkGet(ids).then(items => items.filter((item): item is IMusicTrack => !!item))
  }

  async getAllMusicTracks(): Promise<IMusicTrack[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.imusic_track.toArray()
  }

  async getMusicTrackPage(options: { offset?: number; limit?: number; query?: string } = {}): Promise<IMusicTrack[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    const { offset = 0, limit = 100, query = '' } = options
    const keyword = query.trim().toLowerCase()
    const collection = keyword
      ? this.imusic_track.filter(track => [track.file_name, track.title, track.artist, track.album].some(value => String(value || '').toLowerCase().includes(keyword)))
      : this.imusic_track.orderBy('scanned_at')
    return collection.reverse().offset(offset).limit(limit).toArray()
  }

  async countMusicTracks(query = ''): Promise<number> {
    if (!this.isOpen()) await this.open().catch(() => {})
    const keyword = query.trim().toLowerCase()
    if (!keyword) return this.imusic_track.count()
    return this.imusic_track.filter(track => [track.file_name, track.title, track.artist, track.album].some(value => String(value || '').toLowerCase().includes(keyword))).count()
  }

  async getMusicTracksByDrive(user_id: string, drive_id: string): Promise<IMusicTrack[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.imusic_track.where({ user_id, drive_id }).toArray()
  }

  async deleteMusicTrack(id: string): Promise<void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.imusic_track.delete(id)
  }

  async deleteMusicTracksByIds(ids: string[]): Promise<number> {
    if (!ids || ids.length === 0) return 0
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.imusic_track.bulkDelete(ids).then(() => ids.length).catch(() => 0)
  }

  async deleteMusicTracksByDrive(user_id: string, drive_id: string): Promise<number> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.imusic_track.where({ user_id, drive_id }).delete()
  }

  async clearMusicTracks(): Promise<void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.imusic_track.clear()
  }

  async saveBookItems(books: IBookItem[]): Promise<string | void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    if (!books.length) return
    return this.ibook_item.bulkPut(books).catch(() => {})
  }

  async getAllBookItems(): Promise<IBookItem[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_item.toArray()
  }

  async getBookItemsByIds(ids: string[]): Promise<IBookItem[]> {
    if (!ids.length) return []
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_item.bulkGet(ids).then(items => items.filter((item): item is IBookItem => !!item))
  }

  async getBookItemsByDrive(user_id: string, drive_id: string): Promise<IBookItem[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_item.where({ user_id, drive_id }).toArray()
  }

  async getBookItemsPage(offset: number, limit: number): Promise<IBookItem[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_item.orderBy('scanned_at').reverse().offset(Math.max(0, offset)).limit(Math.max(1, limit)).toArray()
  }

  async getBookItemCounts(): Promise<{ total: number; deleted: number }> {
    if (!this.isOpen()) await this.open().catch(() => {})
    let total = 0
    let deleted = 0
    await this.ibook_item.each((book) => {
      total++
      if (book.deleted_at) deleted++
    })
    return { total, deleted }
  }

  async deleteBookItemsByIds(ids: string[]): Promise<number> {
    if (!ids || ids.length === 0) return 0
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_item.bulkDelete(ids).then(() => ids.length).catch(() => 0)
  }

  async clearBookItems(): Promise<void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_item.clear()
  }

  async countBookItems(): Promise<number> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_item.count()
  }

  async saveBookNotes(notes: IBookNote[]): Promise<string | void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    if (!notes.length) return
    return this.ibook_note.bulkPut(notes).catch(() => {})
  }

  async getBookNotesByBookId(book_id: string): Promise<IBookNote[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_note.where({ book_id }).toArray()
  }

  async getAllBookNotes(): Promise<IBookNote[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_note.toArray()
  }

  async deleteBookNotesByIds(ids: string[]): Promise<number> {
    if (!ids || ids.length === 0) return 0
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_note.bulkDelete(ids).then(() => ids.length).catch(() => 0)
  }

  async deleteBookNotesByBookIds(bookIds: string[]): Promise<number> {
    if (!bookIds || bookIds.length === 0) return 0
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_note.where('book_id').anyOf(bookIds).delete().catch(() => 0)
  }

  async saveBookBookmarks(bookmarks: IBookBookmark[]): Promise<string | void> {
    if (!this.isOpen()) await this.open().catch(() => {})
    if (!bookmarks.length) return
    return this.ibook_bookmark.bulkPut(bookmarks).catch(() => {})
  }

  async getBookBookmarksByBookId(book_id: string): Promise<IBookBookmark[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_bookmark.where({ book_id }).toArray()
  }

  async getAllBookBookmarks(): Promise<IBookBookmark[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_bookmark.toArray()
  }

  async deleteBookBookmarksByIds(ids: string[]): Promise<number> {
    if (!ids || ids.length === 0) return 0
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_bookmark.bulkDelete(ids).then(() => ids.length).catch(() => 0)
  }

  async deleteBookBookmarksByBookIds(bookIds: string[]): Promise<number> {
    if (!bookIds || bookIds.length === 0) return 0
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.ibook_bookmark.where('book_id').anyOf(bookIds).delete().catch(() => 0)
  }
}

const DB = new XBYDB3()
export default DB
