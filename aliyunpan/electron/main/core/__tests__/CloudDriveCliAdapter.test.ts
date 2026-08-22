import { describe, expect, it } from 'vitest'
import { cloudDriveCliScopeArgs, cloudDriveCliStatsArgs, toAgentResourceSnapshot } from '../../agent/CloudDriveCliAdapter.ts'

const scope = { accountId: 'user-1', driveId: 'drive-1', rootId: 'root', platform: 'aliyun', operations: ['files.list'] }

describe('CloudDriveCliAdapter', () => {
  it('binds every CLI inspect command to the approved account and drive', () => {
    expect(cloudDriveCliScopeArgs(scope)).toEqual(['--provider', 'aliyun', '--account', 'aliyun_user-1', '--drive-id', 'drive-1'])
  })

  it('bounds directory statistics and binds them to the approved root', () => {
    expect(cloudDriveCliStatsArgs(scope, 99)).toEqual(['files', 'stats', '--file-id', 'root', '--depth', '2', '--provider', 'aliyun', '--account', 'aliyun_user-1', '--drive-id', 'drive-1'])
  })

  it('rejects a resource returned from another account or drive', () => {
    expect(toAgentResourceSnapshot({ accountId: 'aliyun_user-1', driveId: 'drive-1', fileId: 'file-1', parentFileId: 'root', name: '报告.pdf' }, scope)).toMatchObject({ accountId: 'user-1', resourceId: 'file-1', name: '报告.pdf' })
    expect(() => toAgentResourceSnapshot({ accountId: 'another-user', driveId: 'drive-1', fileId: 'file-1', name: '报告.pdf' }, scope)).toThrow('其他账号')
    expect(() => toAgentResourceSnapshot({ accountId: 'aliyun_user-1', driveId: 'another-drive', fileId: 'file-1', name: '报告.pdf' }, scope)).toThrow('其他网盘')
  })
})
