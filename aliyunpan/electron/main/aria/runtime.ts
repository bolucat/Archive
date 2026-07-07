import { ENGINE_RPC_PORT } from '@shared/constants'

export const getMotrixApplicationRpcPort = (): number => {
  const app = (globalThis as any).motrixApplication
  const port = app?.configManager?.getSystemConfig?.('rpc-listen-port')
  return Number.isFinite(Number(port)) && Number(port) > 0 ? Number(port) : ENGINE_RPC_PORT
}
