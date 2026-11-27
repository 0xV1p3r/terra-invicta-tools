// Shared class strings, so the same kind of surface looks the same on every screen.

/**
 * A floating control over a canvas (zoom, fit, help): opaque so nodes and links don't show
 * through, and the same 32px as the toolbar controls. Use with `variant="outline" size="icon"`.
 */
export const canvasControl =
	'size-8 border-input bg-card shadow-sm hover:bg-accent dark:bg-card dark:hover:bg-accent';

/** A small read-only figure (label above, value below): plan totals, body stats. Outlined. */
export const statTile = 'min-w-0 rounded-md border px-2.5 py-1.5';
/** The label line of a `statTile`. */
export const statLabel = 'truncate text-xs text-muted-foreground';
/** The value line of a `statTile`. */
export const statValue = 'truncate text-sm font-semibold tabular-nums';

/** Header bar of a side panel (plan, bodies, details): same height, padding and border everywhere. */
export const panelHeader =
	'flex min-h-14 shrink-0 items-center justify-between gap-2 border-b px-4 py-2';
/** Title text inside a `panelHeader`. */
export const panelTitle = 'text-base leading-tight font-semibold';
/** Horizontal padding of a side panel's body, matching `panelHeader`. */
export const panelPadding = 'px-4';

/** Section heading in a panel or dialog (Requirements, Goals, …): quiet, sentence case. */
export const sectionHeading = 'text-xs font-medium text-muted-foreground';
/** Sub-heading under a `sectionHeading`: one step lower (regular weight). */
export const subHeading = 'text-xs text-muted-foreground';

/**
 * A standalone chip in a group outside the toolbar (search type chips): selected chips are
 * raised like the active segment of the toolbar's segmented controls, not solid white.
 */
export const chipToggle =
	'h-7 rounded-md border-0 px-2.5 text-sm font-medium text-muted-foreground hover:bg-transparent hover:text-foreground data-[state=on]:bg-raised data-[state=on]:text-foreground data-[state=on]:shadow-[inset_0_0_0_1px_rgb(255_255_255/0.08),0_1px_2px_rgb(0_0_0/0.3)] data-[state=on]:hover:bg-raised data-[state=on]:hover:text-foreground';
