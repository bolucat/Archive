import semver from 'semver'

export function bumpPatchVersion(version) {
  const parsed = semver.parse(version)
  if (!parsed) {
    throw new Error(`Invalid package version: ${version}`)
  }

  const prerelease = parsed.prerelease.length > 0 ? `-${parsed.prerelease.join('.')}` : ''
  return `${parsed.major}.${parsed.minor}.${parsed.patch + 1}${prerelease}`
}
