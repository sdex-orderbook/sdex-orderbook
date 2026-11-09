// SPDX-License-Identifier: Apache-2.0
// AppShell: header (accent rule, monogram, name, gate badges, theme toggle),
// optional sidebar nav, content column and the honesty footer.
// Layouts: "dashboard" (sidebar + wide content) and "report" (single narrow
// column with a masthead). Docs sites use Starlight instead (design/astro/).
import { useCallback, useEffect, useState, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { Monitor, Moon, Sun } from '@phosphor-icons/react';
import { cn } from './cn';

/* ------------------------------------------------------------------ theme */

export type ThemeChoice = 'system' | 'light' | 'dark';
const STORAGE_KEY = 'sik-theme';

/**
 * Inline this in index.html <head> before any stylesheet paints, so the first
 * frame already has the right theme:
 *   <script>{THEME_INIT_SCRIPT}</script>
 * It always writes the RESOLVED theme to data-theme, so `dark:` utilities and
 * tokens agree.
 */
export const THEME_INIT_SCRIPT = `(function(){try{var c=localStorage.getItem('${STORAGE_KEY}');}catch(e){}var d=c==='dark'||((c!=='light')&&window.matchMedia('(prefers-color-scheme: dark)').matches);document.documentElement.setAttribute('data-theme',d?'dark':'light');})();`;

function readChoice(): ThemeChoice {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return v === 'light' || v === 'dark' ? v : 'system';
  } catch {
    return 'system';
  }
}

function apply(choice: ThemeChoice) {
  const dark = choice === 'dark' || (choice === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
  document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light');
}

export function useTheme(): [ThemeChoice, (c: ThemeChoice) => void] {
  const [choice, setChoice] = useState<ThemeChoice>(() => (typeof window === 'undefined' ? 'system' : readChoice()));
  useEffect(() => {
    apply(choice);
    if (choice !== 'system') return;
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => apply('system');
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, [choice]);
  const set = useCallback((c: ThemeChoice) => {
    try {
      if (c === 'system') localStorage.removeItem(STORAGE_KEY);
      else localStorage.setItem(STORAGE_KEY, c);
    } catch {
      /* storage blocked: the choice still applies for this page view */
    }
    setChoice(c);
  }, []);
  return [choice, set];
}

const THEME_OPTIONS: Array<{ value: ThemeChoice; label: string; icon: ReactNode }> = [
  { value: 'system', label: 'System theme', icon: <Monitor weight="regular" /> },
  { value: 'light', label: 'Light theme', icon: <Sun weight="regular" /> },
  { value: 'dark', label: 'Dark theme', icon: <Moon weight="regular" /> },
];

export function ThemeToggle({ className }: { className?: string }) {
  const [choice, setChoice] = useTheme();
  return (
    <div role="radiogroup" aria-label="Colour theme" className={cn('inline-flex rounded-sm border border-line bg-sunken p-0.5', className)}>
      {THEME_OPTIONS.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={choice === o.value}
          aria-label={o.label}
          title={o.label}
          onClick={() => setChoice(o.value)}
          className={cn(
            'grid size-7 place-items-center rounded-[4px] text-[15px] transition-colors duration-[120ms] ease-settle',
            choice === o.value ? 'bg-surface text-ink shadow-1' : 'text-ink-3 hover:text-ink',
          )}
        >
          {o.icon}
        </button>
      ))}
    </div>
  );
}

/* ------------------------------------------------------------- monogram */

/** The project mark: its ticket prefix in mono on the accent wash. */
export function Monogram({ code, size = 'md' }: { code: string; size?: 'sm' | 'md' | 'lg' }) {
  return (
    <span
      aria-hidden
      className={cn(
        'inline-grid shrink-0 place-items-center rounded-xs border border-accent/25 bg-accent-wash font-mono font-semibold text-accent',
        size === 'sm' && 'h-6 min-w-6 px-1 text-[10px]',
        size === 'md' && 'h-8 min-w-8 px-1.5 text-[11px]',
        size === 'lg' && 'h-11 min-w-11 px-2 text-sm',
      )}
    >
      {code}
    </span>
  );
}

/* ---------------------------------------------------------------- shell */

export interface NavItem {
  label: string;
  href: string;
  active?: boolean;
  icon?: ReactNode;
}

export interface AppShellProps {
  project: { name: string; code: string; family?: string; href?: string };
  /** GateBadge elements. Shown in the header on lg+, in a strip below it on smaller screens. */
  gates?: ReactNode;
  nav?: NavItem[];
  layout?: 'dashboard' | 'report';
  /** Extra header actions (left of the theme toggle). */
  actions?: ReactNode;
  /** Footer content in addition to the standard honesty line. */
  footer?: ReactNode;
  /** Hide the theme toggle (e.g. when embedding). */
  hideThemeToggle?: boolean;
  children: ReactNode;
  className?: string;
}

export function AppShell({
  project,
  gates,
  nav,
  layout = 'dashboard',
  actions,
  footer,
  hideThemeToggle = false,
  children,
  className,
}: AppShellProps) {
  const hasSidebar = layout === 'dashboard' && nav && nav.length > 0;
  return (
    <div className={cn('flex min-h-[100dvh] flex-col bg-canvas text-ink', className)}>
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-3 focus:z-[var(--sik-z-toast)] focus:rounded-sm focus:bg-surface focus:px-3 focus:py-2 focus:shadow-2"
      >
        Skip to content
      </a>
      <header className="sticky top-0 z-[var(--sik-z-header)] border-b border-line bg-surface/90 backdrop-blur-md accent-rule">
        <div className="mx-auto flex h-14 max-w-content items-center gap-3 px-4 sm:px-6 lg:px-8">
          <a href={project.href ?? '#'} className="flex min-w-0 items-center gap-2.5 rounded-xs">
            <Monogram code={project.code} />
            <span className="min-w-0">
              <span className="block truncate text-sm font-semibold leading-tight text-ink">{project.name}</span>
              {project.family && <span className="block truncate text-xs leading-tight text-ink-3">{project.family}</span>}
            </span>
          </a>
          {gates && <div className="ml-4 hidden min-w-0 flex-1 items-center gap-1.5 overflow-hidden lg:flex">{gates}</div>}
          <div className="ml-auto flex items-center gap-2">
            {actions}
            {!hideThemeToggle && <ThemeToggle />}
          </div>
        </div>
        {(gates || hasSidebar) && (
          <div className="border-t border-line lg:hidden">
            {gates && <div className="flex gap-1.5 overflow-x-auto px-4 py-2 sm:px-6">{gates}</div>}
            {hasSidebar && (
              <nav aria-label="Sections" className="flex gap-1 overflow-x-auto border-t border-line px-3 py-1.5 sm:px-5">
                {nav.map((n) => (
                  <a
                    key={n.href}
                    href={n.href}
                    aria-current={n.active ? 'page' : undefined}
                    className={cn(
                      'whitespace-nowrap rounded-sm px-2.5 py-1.5 text-sm',
                      n.active ? 'bg-accent-wash font-medium text-accent' : 'text-ink-2 hover:text-ink',
                    )}
                  >
                    {n.label}
                  </a>
                ))}
              </nav>
            )}
          </div>
        )}
      </header>

      <div
        className={cn(
          'mx-auto w-full flex-1 px-4 sm:px-6 lg:px-8',
          layout === 'report' ? 'max-w-report' : 'max-w-content',
          hasSidebar && 'lg:grid lg:grid-cols-[var(--sik-sidebar-w)_minmax(0,1fr)] lg:gap-8',
        )}
      >
        {hasSidebar && (
          <nav aria-label="Sections" className="hidden lg:block">
            <ul role="list" className="sticky top-[calc(var(--sik-header-h)+1.5rem)] mt-6 flex flex-col gap-0.5">
              {nav.map((n) => (
                <li key={n.href}>
                  <a
                    href={n.href}
                    aria-current={n.active ? 'page' : undefined}
                    className={cn(
                      'flex items-center gap-2.5 rounded-sm px-3 py-2 text-sm transition-colors duration-[120ms] ease-settle',
                      n.active ? 'bg-accent-wash font-medium text-accent' : 'text-ink-2 hover:bg-sunken hover:text-ink',
                    )}
                  >
                    {n.icon && <span aria-hidden className="text-base">{n.icon}</span>}
                    {n.label}
                  </a>
                </li>
              ))}
            </ul>
          </nav>
        )}
        <main id="main" className="min-w-0 py-6 lg:py-8">
          {children}
        </main>
      </div>

      <footer className="border-t border-line bg-surface">
        <div
          className={cn(
            'mx-auto flex flex-col gap-2 px-4 py-5 text-xs text-ink-3 sm:flex-row sm:items-center sm:justify-between sm:px-6 lg:px-8',
            layout === 'report' ? 'max-w-report' : 'max-w-content',
          )}
        >
          <p>
            Read-only against Stellar mainnet. No mainnet writes. Every figure shows where it came from.
          </p>
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
            {footer}
            <span className="font-mono">{project.code} · Apache-2.0</span>
          </div>
        </div>
      </footer>
    </div>
  );
}

/* ------------------------------------------------------------ page parts */

export interface PageHeaderProps {
  title: ReactNode;
  /** One sentence, at most about 20 words. */
  lede?: ReactNode;
  /** Provenance chip or gate badges for the whole page. */
  meta?: ReactNode;
  actions?: ReactNode;
}

/** Top of a dashboard view or report: title, one-line lede, meta row. */
export function PageHeader({ title, lede, meta, actions }: PageHeaderProps) {
  return (
    <div className="mb-6 flex flex-col gap-4 border-b border-line pb-6 md:flex-row md:items-end md:justify-between">
      <div className="min-w-0 max-w-prose">
        <h1 className="text-2xl font-semibold tracking-tight text-ink">{title}</h1>
        {lede && <p className="mt-2 text-md text-ink-2">{lede}</p>}
        {meta && <div className="mt-3 flex flex-wrap items-center gap-2">{meta}</div>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}

export function Button({
  variant = 'secondary',
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: 'primary' | 'secondary' | 'ghost' }) {
  return (
    <button
      type="button"
      {...props}
      className={cn(
        'inline-flex h-8 items-center gap-1.5 whitespace-nowrap rounded-sm px-3 text-sm font-medium',
        'transition-[background-color,border-color,color,transform] duration-[120ms] ease-settle active:translate-y-px',
        'disabled:pointer-events-none disabled:opacity-50',
        variant === 'primary' && 'bg-accent text-ink-inverse inset-shadow-highlight hover:brightness-110',
        variant === 'secondary' && 'border border-line-strong bg-surface text-ink hover:bg-sunken',
        variant === 'ghost' && 'text-ink-2 hover:bg-sunken hover:text-ink',
        className,
      )}
    />
  );
}
