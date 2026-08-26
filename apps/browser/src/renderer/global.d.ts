import type { ArchetypeRuntime } from '@archetype/protocol'
import type { ShellBridge } from '../shared.js'

declare global {
  interface Window { archetypeShell: ShellBridge }
  interface Navigator { archetype: ArchetypeRuntime }
}

export {}
