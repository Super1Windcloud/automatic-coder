import path from 'node:path'
import { defineConfig } from 'vite'

export default defineConfig({
  base: './',
  build: {
    outDir: path.resolve(__dirname, 'dist/activation'),
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: path.resolve(__dirname, 'activation.html'),
      },
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
})
