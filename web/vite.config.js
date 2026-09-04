import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

export default defineConfig({
  plugins: [svelte()],
  build: {
    outDir: 'dist',
    target: 'es2022',
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./vitest-setup.js'],
    server: {
      deps: {
        // svelte 5 client runtime, bukan server runtime
        inline: [/svelte/],
      },
    },
    alias: {
      // paksa resolusi browser untuk svelte internals
      'svelte': 'svelte',
    },
  },
  resolve: {
    conditions: ['browser'],
  },
})
