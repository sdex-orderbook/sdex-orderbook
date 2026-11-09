// SPDX-License-Identifier: Apache-2.0
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './main.css';
import { App } from './App';
import type { DemoData } from './lib/types';

const root = document.getElementById('root');
if (!root) throw new Error('#root missing');
// demo.json (built by `pnpm demo:data` from committed fixtures) is its own chunk.
const { default: data } = await import('./data/demo.json');
createRoot(root).render(
  <StrictMode>
    <App data={data as unknown as DemoData} />
  </StrictMode>,
);
