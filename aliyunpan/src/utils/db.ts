import Dexie from 'dexie'
import { ITokenInfo } from '../user/userstore'
import { IOtherShareLinkModel } from '../share/share/OtherShareStore'
import { IMusicTrack } from '../types/music'
import { IBookItem } from '../types/book'
import { IBookNote } from '../types/bookNote'
import { IBookBookmark } from '../types/bookBookmark'
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

  async getAllMusicTracks(): Promise<IMusicTrack[]> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.imusic_track.toArray()
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

  async countMusicTracks(): Promise<number> {
    if (!this.isOpen()) await this.open().catch(() => {})
    return this.imusic_track.count()
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
