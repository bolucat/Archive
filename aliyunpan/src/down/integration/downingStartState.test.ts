import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const batchResumeTasks = vi.fn()

vi.mock('./aria2TaskApi', () => ({
  batchPauseTasks: vi.fn(),
  batchResumeTasks,
  batchRemoveTasks: vi.fn()
}))

vi.mock('../../utils/message', () => ({
  default: {
    info: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    success: vi.fn()
  }
}))

vi.mock('../../utils/dbdown', () => ({
  default: {
    saveDownings: vi.fn(),
    deleteDownings: vi.fn(),
    deleteDowningAll: vi.fn()
  }
}))

vi.mock('../DownDAL', () => ({
  default: {
    stopDowning: vi.fn(),
    deleteDowning: vi.fn(),
    deleteDowned: vi.fn()
  }
}))

describe('DowningStore start state', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    batchResumeTasks.mockReset()
  })

  it('resumes a paused aria task when selected task is started', async () => {
    const { default: useDowningStore } = await import('../DowningStore')
    const store = useDowningStore()
    store.ListDataRaw = [{
      DownID: 'task-1',
      Info: {
        GID: 'gid-1',
        user_id: 'external',
        DownSavePath: '/tmp',
        ariaRemote: false,
        file_id: 'file-1',
        drive_id: 'external',
        name: 'movie.torrent',
        size: 0,
        sizestr: '',
        icon: 'iconfile-bt',
        isDir: false,
        encType: '',
        sha1: '',
        crc64: '',
        sourceType: 'torrent'
      },
      Down: {
        DownState: '已暂停',
        DownTime: 1,
        DownSize: 0,
        DownSpeed: 0,
        DownSpeedStr: '',
        DownProcess: 0,
        IsStop: true,
        IsDowning: false,
        IsCompleted: false,
        IsFailed: false,
        FailedCode: 0,
        FailedMessage: '',
        AutoTry: 0,
        DownUrl: ''
      }
    }]
    store.ListSelected = new Set(['task-1'])

    await store.mStartDowning()

    expect(store.ListDataRaw[0].Down.DownState).toBe('队列中')
    expect(batchResumeTasks).toHaveBeenCalledWith(['gid-1'])
  })
})
