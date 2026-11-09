// SPDX-License-Identifier: Apache-2.0
// Asset strings as sdx and the diff fixtures write them: "native" or "CODE:ISSUER".

export interface ParsedAsset {
  type: 'native' | 'credit_alphanum4' | 'credit_alphanum12';
  code: string;
  issuer: string | null;
}

export function parseAsset(s: string): ParsedAsset {
  if (s === 'native') return { type: 'native', code: 'XLM', issuer: null };
  const i = s.indexOf(':');
  if (i <= 0 || i === s.length - 1) throw new Error(`bad asset string: ${s}`);
  const code = s.slice(0, i);
  const issuer = s.slice(i + 1);
  if (code.length > 12) throw new Error(`asset code too long: ${s}`);
  return { type: code.length <= 4 ? 'credit_alphanum4' : 'credit_alphanum12', code, issuer };
}

/** Short label: "XLM" for native, otherwise the asset code. */
export function assetLabel(s: string): string {
  return parseAsset(s).code;
}

/** Horizon query parameters for an asset under a prefix ("selling", "base", ...). */
export function assetParams(prefix: string, s: string): string {
  const a = parseAsset(s);
  if (a.type === 'native') return `${prefix}_asset_type=native`;
  return `${prefix}_asset_type=${a.type}&${prefix}_asset_code=${encodeURIComponent(a.code)}&${prefix}_asset_issuer=${a.issuer}`;
}

/** Label from a Horizon path-record asset object. */
export function horizonAssetLabel(a: { asset_type: string; asset_code?: string }): string {
  return a.asset_type === 'native' ? 'XLM' : (a.asset_code ?? '?');
}

/** Pair label "XLM / USDC" (base / counter). */
export function pairLabel(base: string, counter: string): string {
  return `${assetLabel(base)} / ${assetLabel(counter)}`;
}
