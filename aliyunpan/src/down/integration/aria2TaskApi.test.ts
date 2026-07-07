import { describe, expect, it, vi } from 'vitest'
import { normalizeAriaTask, normalizeTaskFiles } from './taskTypes'

vi.mock('../../utils/aria2c', () => ({
  AriaConnect: vi.fn(),
  AriaRawCall: vi.fn()
}))

import { AriaRawCall } from '../../utils/aria2c'
import { batchPauseTasks, batchRemoveTasks, batchResumeTasks, buildSelectFileOption, normalizeTaskListResult } from './aria2TaskApi'

describe('normalizeAriaTask', () => {
  it('normalizes aria2 task numbers that arrive as strings', () => {
    const task = normalizeAriaTask({
      gid: 'abc', status: 'active', totalLength: '2048',
      completedLength: '1024', downloadSpeed: '512', uploadSpeed: '128',
      numSeeders: '2', seeder: 'true', dir: '/tmp/downloads', files: []
    })
    expect(task).toMatchObject({
      gid: 'abc', status: 'active', totalLength: 2048,
      completedLength: 1024, downloadSpeed: 512, uploadSpeed: 128,
      numSeeders: 2, seeder: true
    })
  })
})

describe('normalizeTaskFiles', () => {
  it('normalizes file index, length, selected, and adds name', () => {
    const files = normalizeTaskFiles([
      { index: '1', path: '/tmp/a.mkv', length: '100', completedLength: '50', selected: 'true' },
      { index: '2', path: '/tmp/b.srt', length: '10', completedLength: '0', selected: 'false' }
    ])
    expect(files).toEqual([
      { index: 1, path: '/tmp/a.mkv', name: 'a.mkv', length: 100, completedLength: 50, selected: true },
      { index: 2, path: '/tmp/b.srt', name: 'b.srt', length: 10, completedLength: 0, selected: false }
    ])
  })
})

describe('aria2TaskApi helpers', () => {
  it('builds aria2 select-file option from selected file indexes', () => {
    expect(buildSelectFileOption([0, 2, Number.NaN, 4, -1])).toEqual({ 'select-file': '2,4' })
  })

  it('normalizes task list results returned by aria2', () => {
    const list = normalizeTaskListResult([
      { gid: 'g1', totalLength: '9', completedLength: '3', files: [{ index: '1', path: '/tmp/a.bin', length: '9', selected: 'true' }] }
    ])

    expect(list[0].totalLength).toBe(9)
    expect(list[0].files[0]).toMatchObject({ index: 1, name: 'a.bin', selected: true })
  })

  it('pauses selected tasks through Download aria2 task API', async () => {
    vi.mocked(AriaRawCall).mockResolvedValue(undefined)

    await batchPauseTasks(['g1', 'g2'])

    expect(AriaRawCall).toHaveBeenCalledWith('aria2.forcePause', 'g1')
    expect(AriaRawCall).toHaveBeenCalledWith('aria2.forcePause', 'g2')
  })

  it('resumes selected tasks through Download aria2 task API', async () => {
    vi.mocked(AriaRawCall).mockResolvedValue(undefined)

    await batchResumeTasks(['g1', 'g2'])

    expect(AriaRawCall).toHaveBeenCalledWith('aria2.unpause', 'g1')
    expect(AriaRawCall).toHaveBeenCalledWith('aria2.unpause', 'g2')
  })

  it('removes selected tasks and aria2 results through Download aria2 task API', async () => {
    vi.mocked(AriaRawCall).mockResolvedValue(undefined)

    await batchRemoveTasks(['g1'])

    expect(AriaRawCall).toHaveBeenCalledWith('aria2.forceRemove', 'g1')
    expect(AriaRawCall).toHaveBeenCalledWith('aria2.removeDownloadResult', 'g1')
  })
})
