import { defineConfig } from 'vitest/config'

export default defineConfig({
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
