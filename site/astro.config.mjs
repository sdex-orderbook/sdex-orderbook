// SPDX-License-Identifier: Apache-2.0
// sdx docs (Starlight, re-themed with the kit tokens). Started from
// design/astro/astro.config.example.mjs. Pinned 2026-11-08: astro 7.4.2,
// @astrojs/starlight 0.42.7, @fontsource-variable/geist{,-mono} 5.3.0.
// Served under /docs/ next to the web app (DEMO.md, DEPLOY.md); override with SDX_DOCS_BASE.
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

const PROJECT = 'sdex-orderbook';

export default defineConfig({
  base: process.env.SDX_DOCS_BASE ?? '/docs',
  integrations: [
    starlight({
      title: 'sdx docs',
      description: 'Horizon-compatible SDEX order book, path finding and trade aggregations rebuilt from Stellar ledger data. Read-only, unaudited.',
      head: [{ tag: 'script', content: `document.documentElement.dataset.project=${JSON.stringify(PROJECT)}` }],
      customCss: ['@fontsource-variable/geist', '@fontsource-variable/geist-mono', './src/styles/kit/tokens.css', './src/styles/kit/starlight-theme.css'],
      components: { SiteTitle: './src/components/kit/SiteTitle.astro' },
      sidebar: [
        { label: 'Start', items: [{ label: 'Overview', link: '/' }, { label: 'Quickstart', slug: 'quickstart' }] },
        { label: 'Reference', items: [{ label: 'CLI', slug: 'cli' }, { label: 'HTTP API', slug: 'http-api' }, { label: 'State directory', slug: 'state' }] },
        { label: 'Evidence', items: [{ label: 'Differential check', slug: 'diff' }, { label: 'Status and limits', slug: 'limits' }] },
      ],
      expressiveCode: {
        themes: ['github-dark-default', 'github-light-default'],
        styleOverrides: { borderRadius: '6px', borderColor: 'var(--sik-line)', codeFontFamily: 'var(--sik-font-mono)', uiFontFamily: 'var(--sik-font-sans)' },
      },
    }),
  ],
});
