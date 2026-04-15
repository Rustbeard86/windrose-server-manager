import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  build: {
    // Output built assets to the backend's static directory so
    // `cargo run` from the backend folder serves the compiled UI.
    outDir: resolve(__dirname, '../static'),
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    // During development, proxy API and WebSocket calls to the local backend.
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8787',
        changeOrigin: true,
      },
      '/ws': {
        target: 'ws://127.0.0.1:8787',
        ws: true,
      },
    },
  },
})
