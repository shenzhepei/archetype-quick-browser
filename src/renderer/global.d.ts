import type { ArchetypeBridge } from '../shared/browser'

declare module '*.scss'

declare global {
  interface Window {
    archetype?: ArchetypeBridge
  }
}

export {}
