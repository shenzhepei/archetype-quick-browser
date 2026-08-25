import { resolve } from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  root: 'src/renderer',
  base: process.env.VITE_BASE_PATH ?? '/',
  plugins: [react()],
  build: {
    outDir: resolve('out/renderer'),
    emptyOutDir: true
  }
})
