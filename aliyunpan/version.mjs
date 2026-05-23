import fs from 'node:fs'
import { bumpPatchVersion } from './scripts/version-utils.mjs'

const packageJsonPath = './package.json'

try {
  const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'))
  const nextVersion = bumpPatchVersion(packageJson.version)

  packageJson.version = nextVersion
  fs.writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`)
  console.info(`版本升级为 ${nextVersion}`)
} catch (e) {
  console.error('处理 package.json 失败，请重试', e instanceof Error ? e.message : e)
  process.exit(1)
}
