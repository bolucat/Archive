import { describe, expect, it } from 'vitest'
import { resolveAriaProgressErrorState } from './downloadProgressState'

describe('resolveAriaProgressErrorState', () => {
  it('clears stale error state when aria reports a running task', () => {
    const state = resolveAriaProgressErrorState(
      {
        status: 'active',
        errorCode: '',
        errorMessage: ''
      },
      () => '创建 Aria 任务失败连接断开'
    )

    expect(state).toEqual({
      isFailed: false,
      failedCode: 0,
      failedMessage: ''
    })
  })

  it('formats aria errors when aria reports an error status', () => {
    const state = resolveAriaProgressErrorState(
      {
        status: 'error',
        errorCode: '19',
        errorMessage: 'Name resolution failed'
      },
      (code, message) => `${code}:${message}`
    )

    expect(state).toEqual({
      isFailed: true,
      failedCode: 19,
      failedMessage: '19:Name resolution failed'
    })
  })
})
