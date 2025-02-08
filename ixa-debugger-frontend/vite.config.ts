import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: '../static', // Ensure Rust serves this
  },
  server: {
    proxy: {
      "/config.json": "http://127.0.0.1:33334",
      "/api": "http://127.0.0.1:33334",
    }
  }
})
