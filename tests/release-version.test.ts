import { isReleaseNewer } from '../src/shared/browser'

it('compares stable GitHub release versions', () => {
  expect(isReleaseNewer('v1.3.0', '1.2.9')).toBe(true)
  expect(isReleaseNewer('1.2.9', '1.3.0')).toBe(false)
  expect(isReleaseNewer('v1.2.3', '1.2.3')).toBe(false)
})

it('handles prerelease and invalid tags', () => {
  expect(isReleaseNewer('1.2.3', '1.2.3-beta.1')).toBe(true)
  expect(isReleaseNewer('latest', '1.2.3')).toBeUndefined()
})
