import type { ArchetypeRuntime, InvokeOptions } from '@archetype/protocol'

export type OperationMap = Record<string, { input: unknown; output: unknown }>

export function createClient<Operations extends OperationMap>(runtime: ArchetypeRuntime) {
  return {
    invoke<Name extends keyof Operations & string>(name: Name, input: Operations[Name]['input'], options?: InvokeOptions): Promise<Operations[Name]['output']> {
      return runtime.invoke(name, input, options)
    }
  }
}
