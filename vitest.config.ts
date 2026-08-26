import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  resolve: {
    alias: {
      '@archetype/browser-runtime': fileURLToPath(new URL('./packages/browser-runtime/src/index.ts', import.meta.url)),
      '@archetype/db-adapters': fileURLToPath(new URL('./packages/db-adapters/src/index.ts', import.meta.url)),
      '@archetype/function-sdk': fileURLToPath(new URL('./packages/function-sdk/src/index.ts', import.meta.url)),
      '@archetype/generated-client': fileURLToPath(new URL('./packages/generated-client/src/index.ts', import.meta.url)),
      '@archetype/protocol': fileURLToPath(new URL('./packages/protocol/src/index.ts', import.meta.url))
    }
  },
  test: {
    include: ['packages/**/*.test.ts', 'services/**/*.test.ts', 'examples/**/*.test.ts', 'apps/**/*.test.{ts,tsx}'],
    environment: 'node',
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      reportsDirectory: 'coverage',
      include: ['packages/*/src/**/*.ts', 'services/*/src/**/*.ts'],
      exclude: ['**/*.d.ts', '**/index.ts']
    }
  }
})
