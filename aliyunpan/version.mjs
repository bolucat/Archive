import fs from 'node:fs'
import { bumpPatchVersion } from './scripts/version-utils.mjs'

const packageJsonPath = './package.json'

try {
  if (process.env.SKIP_VERSION_BUMP === 'true') {
    const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'))
    console.info(`跳过版本升级，当前版本为 ${packageJson.version}`)
    process.exit(0)
  }

  const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'))
  const nextVersion = bumpPatchVersion(packageJson.version)

  packageJson.version = nextVersion
  fs.writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`)
  console.info(`版本升级为 ${nextVersion}`)
} catch (e) {
  console.error('处理 package.json 失败，请重试', e instanceof Error ? e.message : e)
  process.exit(1)
}
