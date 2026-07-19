import { describe, expect, it } from 'vitest'
import { unsupportedAgentToolMessage } from '../../services/agent/agentToolCapabilities'
import { getProviderCapabilities } from '../../services/agent/providerCapabilities'

describe('agent tool capability guard', () => {
  it('allows a declared capability', () => {
    expect(unsupportedAgentToolMessage(getProviderCapabilities('aliyun'), 'downloadFiles')).toBeNull()
  })

  it('blocks a tool when its provider capability is disabled', () => {
    const message = unsupportedAgentToolMessage(getProviderCapabilities('onedrive'), 'exportDirectLinks')
    expect(message).toContain('OneDrive')
    expect(message).toContain('exportDirectLinks')
  })

  it('keeps read-only WebDAV out of destructive tools', () => {
    expect(unsupportedAgentToolMessage(getProviderCapabilities('webdav'), 'deleteFiles')).toContain('WebDAV')
  })
})
