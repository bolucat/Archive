import { describe, expect, it } from 'vitest'
import { vi } from 'vitest'

vi.hoisted(() => {
  ;(globalThis as any).self = globalThis
})

vi.mock('../../utils/format', () => ({ Sleep: vi.fn() }))

import {
  buildDropboxChildPath,
  buildDropboxRelocationBatchBody,
  buildDropboxRelocationBody,
  extractDropboxPathFromDescription,
  parentPathFromDropboxPath,
  resolveDropboxCommandPath
} from '../filecmd'

describe('Dropbox file command helpers', () => {
  it('extracts encoded Dropbox paths from file descriptions', () => {
    expect(extractDropboxPathFromDescription('dropbox_path:%2FMovies%2Fdemo.mkv;dropbox_rev:abc')).toBe('/Movies/demo.mkv')
    expect(extractDropboxPathFromDescription('')).toBe('')
  })

  it('resolves root and path-like ids for command endpoints', () => {
    expect(resolveDropboxCommandPath('dropbox_root')).toBe('')
    expect(resolveDropboxCommandPath('/Movies/demo.mkv')).toBe('/Movies/demo.mkv')
    expect(resolveDropboxCommandPath('id:abc', 'dropbox_path:%2FMovies%2Fdemo.mkv')).toBe('/Movies/demo.mkv')
    expect(resolveDropboxCommandPath('id:abc')).toBe('id:abc')
  })

  it('builds child paths under root and nested folders', () => {
    expect(buildDropboxChildPath('dropbox_root', 'New Folder')).toBe('/New Folder')
    expect(buildDropboxChildPath('/Movies', 'demo.mkv')).toBe('/Movies/demo.mkv')
    expect(buildDropboxChildPath('id:folder', 'demo.mkv', 'dropbox_path:%2FMovies')).toBe('/Movies/demo.mkv')
    expect(buildDropboxChildPath('id:folder', 'New Folder', 'dropbox_path:%2FMovies')).toBe('/Movies/New Folder')
  })

  it('derives a parent path from a Dropbox path', () => {
    expect(parentPathFromDropboxPath('/Movies/demo.mkv')).toBe('/Movies')
    expect(parentPathFromDropboxPath('/demo.mkv')).toBe('')
    expect(parentPathFromDropboxPath('id:abc')).toBe('')
  })

  it('builds a move/copy body using autorename', () => {
    expect(buildDropboxRelocationBody('/Movies/demo.mkv', '/Archive/demo.mkv')).toEqual({
      from_path: '/Movies/demo.mkv',
      to_path: '/Archive/demo.mkv',
      allow_shared_folder: true,
      autorename: true,
      allow_ownership_transfer: false
    })
  })

  it('builds official v2 batch relocation bodies', () => {
    expect(buildDropboxRelocationBatchBody([{ from_path: '/Movies/a.mp4', to_path: '/Archive/a.mp4' }], true)).toEqual({
      entries: [{ from_path: '/Movies/a.mp4', to_path: '/Archive/a.mp4' }],
      autorename: true,
      allow_ownership_transfer: false
    })
  })
})
