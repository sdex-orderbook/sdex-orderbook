// SPDX-License-Identifier: Apache-2.0
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

// Static build (dist/), relative base so it can be served from any path.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: './',
  build: { chunkSizeWarningLimit: 900 },
});
