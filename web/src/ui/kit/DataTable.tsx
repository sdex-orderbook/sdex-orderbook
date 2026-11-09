// SPDX-License-Identifier: Apache-2.0
// DataTable: sortable, sticky-header table for measured rows. Numbers are
// right-aligned tabular figures; hashes are mono and middle-truncated with the
// full value in the tooltip. Row separators are single hairlines. DESIGN.md 8.5.
import { useMemo, useState, type ReactNode } from 'react';
import { CaretDown, CaretUp, CaretUpDown } from '@phosphor-icons/react';
import { cn, truncateMiddle } from './cn';

export interface Column<T> {
  key: string;
  header: ReactNode;
  cell: (row: T) => ReactNode;
  /** Enables sorting on this column. */
  sortValue?: (row: T) => number | string;
  /** Numbers: right-aligned, tabular figures. */
  numeric?: boolean;
  /** Hashes, ids, addresses: mono. */
  mono?: boolean;
  align?: 'left' | 'right' | 'center';
  /** CSS width, e.g. "12rem" or "30%". */
  width?: string;
  /** Hide below the md breakpoint (keep the essential columns on phones). */
  hideOnMobile?: boolean;
}

export interface DataTableProps<T> {
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  /** Visible caption above the table: what the rows are. */
  caption?: ReactNode;
  /** Right side of the caption bar: provenance chip, filters, export. */
  meta?: ReactNode;
  initialSort?: { key: string; dir: 'asc' | 'desc' };
  dense?: boolean;
  /** Fixed max height in px; the header sticks while the body scrolls. */
  maxHeight?: number;
  /** Rendered instead of the body when rows is empty (use EmptyState). */
  empty?: ReactNode;
  footer?: ReactNode;
  onRowClick?: (row: T) => void;
  className?: string;
}

export function DataTable<T>({
  columns,
  rows,
  rowKey,
  caption,
  meta,
  initialSort,
  dense = false,
  maxHeight,
  empty,
  footer,
  onRowClick,
  className,
}: DataTableProps<T>) {
  const [sort, setSort] = useState(initialSort);

  const sorted = useMemo(() => {
    const col = sort && columns.find((c) => c.key === sort.key);
    if (!sort || !col?.sortValue) return rows;
    const get = col.sortValue;
    const out = [...rows].sort((a, b) => {
      const x = get(a);
      const y = get(b);
      const r = typeof x === 'number' && typeof y === 'number' ? x - y : String(x).localeCompare(String(y));
      return sort.dir === 'asc' ? r : -r;
    });
    return out;
  }, [rows, columns, sort]);

  const toggle = (key: string) =>
    setSort((s) => (s?.key === key ? { key, dir: s.dir === 'asc' ? 'desc' : 'asc' } : { key, dir: 'desc' }));

  const pad = dense ? 'px-3 py-1.5' : 'px-4 py-2.5';
  const alignOf = (c: Column<T>) => c.align ?? (c.numeric ? 'right' : 'left');

  return (
    <div className={cn('min-w-0 overflow-hidden rounded-md border border-line bg-surface', className)}>
      {(caption || meta) && (
        <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 border-b border-line px-4 py-3">
          {caption && <h3 className="text-sm font-semibold text-ink">{caption}</h3>}
          {meta && <div className="flex flex-wrap items-center gap-2">{meta}</div>}
        </div>
      )}
      <div className="overflow-auto" style={maxHeight ? { maxHeight } : undefined}>
        <table className="w-full border-collapse text-sm">
          <thead className="sticky top-0 z-[var(--sik-z-sticky)] bg-sunken">
            <tr>
              {columns.map((c) => {
                const active = sort?.key === c.key;
                const a = alignOf(c);
                return (
                  <th
                    key={c.key}
                    scope="col"
                    style={c.width ? { width: c.width } : undefined}
                    aria-sort={active ? (sort.dir === 'asc' ? 'ascending' : 'descending') : undefined}
                    className={cn(
                      'border-b border-line font-medium whitespace-nowrap text-ink-2 text-xs',
                      pad,
                      a === 'right' ? 'text-right' : a === 'center' ? 'text-center' : 'text-left',
                      c.hideOnMobile && 'hidden md:table-cell',
                    )}
                  >
                    {c.sortValue ? (
                      <button
                        type="button"
                        onClick={() => toggle(c.key)}
                        className={cn(
                          'inline-flex items-center gap-1 rounded-xs transition-colors duration-[120ms] ease-settle hover:text-ink',
                          a === 'right' && 'flex-row-reverse',
                          active && 'text-ink',
                        )}
                      >
                        {c.header}
                        <span aria-hidden className={active ? 'text-accent' : 'text-ink-3'}>
                          {active ? sort.dir === 'asc' ? <CaretUp weight="bold" /> : <CaretDown weight="bold" /> : <CaretUpDown />}
                        </span>
                      </button>
                    ) : (
                      c.header
                    )}
                  </th>
                );
              })}
            </tr>
          </thead>
          {sorted.length > 0 && (
            <tbody>
              {sorted.map((row) => (
                <tr
                  key={rowKey(row)}
                  onClick={onRowClick ? () => onRowClick(row) : undefined}
                  className={cn(
                    'border-b border-line last:border-b-0 transition-colors duration-[120ms] ease-settle hover:bg-sunken/70',
                    onRowClick && 'cursor-pointer',
                  )}
                >
                  {columns.map((c) => {
                    const a = alignOf(c);
                    return (
                      <td
                        key={c.key}
                        className={cn(
                          pad,
                          'align-middle text-ink',
                          a === 'right' ? 'text-right' : a === 'center' ? 'text-center' : 'text-left',
                          c.numeric && 'tabular whitespace-nowrap',
                          c.mono && 'font-mono text-xs',
                          c.hideOnMobile && 'hidden md:table-cell',
                        )}
                      >
                        {c.cell(row)}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          )}
        </table>
        {sorted.length === 0 && <div className="p-4">{empty ?? <p className="text-sm text-ink-3">No rows.</p>}</div>}
      </div>
      {footer && <div className="border-t border-line px-4 py-2.5 text-xs text-ink-3">{footer}</div>}
    </div>
  );
}

/** Mono, middle-truncated hash with the full value in the tooltip and for screen readers. */
export function Hash({ value, head = 8, tail = 6, href }: { value: string; head?: number; tail?: number; href?: string }) {
  const shown = truncateMiddle(value, head, tail);
  const inner = (
    <>
      <span aria-hidden>{shown}</span>
      <span className="sr-only">{value}</span>
    </>
  );
  return href ? (
    <a href={href} title={value} className="font-mono text-accent underline-offset-2 hover:underline">
      {inner}
    </a>
  ) : (
    <span title={value} className="font-mono">
      {inner}
    </span>
  );
}
