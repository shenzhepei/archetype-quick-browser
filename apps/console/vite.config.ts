import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  base: '/console/',
  plugins: [react()],
  server: { proxy: { '/v1': 'http://localhost:8787' } }
})
