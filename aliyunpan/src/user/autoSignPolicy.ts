import type { ITokenInfo } from './userstore'

export function supportsAliyunAutoSign(token: Pick<ITokenInfo, 'tokenfrom'>): boolean {
  return token.tokenfrom === 'aliyun' || token.tokenfrom === 'unknown'
}
