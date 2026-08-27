import { describe, expect, test } from 'vitest'
import { hasControlPermission } from './control-auth.js'

describe('control-plane roles', () => {
  test('limits membership management to owners and administrators', () => {
    expect(hasControlPermission('owner', 'member:write')).toBe(true)
    expect(hasControlPermission('admin', 'member:write')).toBe(true)
    expect(hasControlPermission('developer', 'member:write')).toBe(false)
    expect(hasControlPermission('operator', 'member:write')).toBe(false)
    expect(hasControlPermission('auditor', 'member:write')).toBe(false)
  })

  test('keeps auditors read-only and separates deployment from operations', () => {
    expect(hasControlPermission('auditor', 'project:read')).toBe(true)
    expect(hasControlPermission('auditor', 'project:configure')).toBe(false)
    expect(hasControlPermission('operator', 'project:configure')).toBe(true)
    expect(hasControlPermission('operator', 'deployment:write')).toBe(false)
    expect(hasControlPermission('developer', 'deployment:write')).toBe(true)
  })
})
