// SPDX-License-Identifier: Apache-2.0
// Barrel for the reference components. Copy the whole folder into a project
// (e.g. src/ui/kit/) and import from here.
export { cn, formatCompact, formatExact, formatLedger, truncateMiddle } from './cn';
export { ProvenanceChip, type Provenance, type ProvenanceKind, type ProvenanceChipProps } from './ProvenanceChip';
export { GateBadge, GateNotice, GZKNotice, TONE_CLASSES, type GateKind, type GateBadgeProps, type Tone } from './GateBadge';
export { EmptyState, UnknownValue, Skeleton, type EmptyTone, type UnknownKind } from './EmptyState';
export { StatTile, Sparkline, type StatTileProps, type Delta } from './StatTile';
export { DataTable, Hash, type Column, type DataTableProps } from './DataTable';
export { StatusTable, StatusLabel, type Status, type StatusRow } from './StatusTable';
export {
  AppShell,
  PageHeader,
  Button,
  Monogram,
  ThemeToggle,
  useTheme,
  THEME_INIT_SCRIPT,
  type AppShellProps,
  type NavItem,
  type ThemeChoice,
} from './AppShell';
export { ProjectHero, type ProjectHeroProps, type SignatureFigure } from './ProjectHero';
export { ChartFrame, RankedBars, type Bar, type ChartFrameProps } from './ChartFrame';
