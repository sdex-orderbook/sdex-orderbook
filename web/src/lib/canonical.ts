// SPDX-License-Identifier: Apache-2.0
// Canonical JSON (object keys sorted by UTF-16 code units, no whitespace) so a
// hash of a parsed body does not depend on key order. The demo shows the
// SHA-256 of the canonical form of exactly the fields `sdx diff` compares.

export function canonicalJson(v: unknown): string {
  if (v === null || typeof v === 'boolean' || typeof v === 'number' || typeof v === 'string') return JSON.stringify(v);
  if (Array.isArray(v)) return `[${v.map(canonicalJson).join(',')}]`;
  if (typeof v === 'object') {
    const o = v as Record<string, unknown>;
    const keys = Object.keys(o).sort();
    return `{${keys.map((k) => `${JSON.stringify(k)}:${canonicalJson(o[k])}`).join(',')}}`;
  }
  throw new Error(`canonicalJson: unsupported value of type ${typeof v}`);
}

/** Deep equality on parsed JSON values (same semantics as serde_json::Value ==). */
export function jsonEqual(a: unknown, b: unknown): boolean {
  return canonicalJson(a) === canonicalJson(b);
}
