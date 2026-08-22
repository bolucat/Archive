export interface AgentV1FeatureFlags {
  controlPlane: boolean
  workspaceBridge: boolean
  workspaceCutover: boolean
  mediaAcquisitionBridge: boolean
  aiSearchBridge: boolean
  aiSearchCutover: boolean
  documentBridge: boolean
  cliExecutor: boolean
}

export function agentV1FeatureFlags(env: Record<string, string | undefined> = process.env): AgentV1FeatureFlags {
  const controlPlane = env.BOXPLAYER_AGENT_V1 === '1'
  const workspaceBridge = controlPlane && env.BOXPLAYER_AGENT_V1_WORKSPACE === '1'
  const workspaceCutover = workspaceBridge && env.BOXPLAYER_AGENT_V1_EXECUTOR === '1' && env.BOXPLAYER_AGENT_V1_WORKSPACE_CUTOVER === '1'
  const aiSearchBridge = controlPlane && env.BOXPLAYER_AGENT_V1_AI_SEARCH === '1'
  const aiSearchCutover = aiSearchBridge && env.BOXPLAYER_AGENT_V1_EXECUTOR === '1' && env.BOXPLAYER_AGENT_V1_AI_SEARCH_CUTOVER === '1'
  return {
    controlPlane,
    workspaceBridge,
    workspaceCutover,
    mediaAcquisitionBridge: controlPlane && env.BOXPLAYER_AGENT_V1_MEDIA === '1',
    aiSearchBridge,
    aiSearchCutover,
    documentBridge: controlPlane && env.BOXPLAYER_AGENT_V1_DOCUMENT === '1',
    // Shadow-mirrored bridges are still executed by their legacy runners.
    // Start the executor only when at least one surface explicitly owns cutover.
    cliExecutor: controlPlane && env.BOXPLAYER_AGENT_V1_EXECUTOR === '1' && (workspaceCutover || aiSearchCutover)
  }
}
