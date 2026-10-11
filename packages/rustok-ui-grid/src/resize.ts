/**
 * Column resizing: persisted widths and pointer arithmetic for RusTok DataGrid —
 * the TypeScript twin of `crates/ui/rustok-grid/src/resize.rs`.
 *
 * Framework-free: defensive math handling non-finite numbers and reversed bounds.
 */

/** Default pixel step when resizing columns with keyboard arrow keys. */
export const KEYBOARD_RESIZE_STEP = 16;

/**
 * Stores user-overridden column widths, keyed by column id.
 */
export class ColumnWidths {
  private readonly widths: Map<string, number>;

  constructor(initial?: Record<string, number> | Map<string, number>) {
    if (initial instanceof Map) {
      this.widths = new Map(initial);
    } else if (initial) {
      this.widths = new Map(Object.entries(initial));
    } else {
      this.widths = new Map();
    }
  }

  /** Stored override, if the user ever resized this column. */
  getStored(columnId: string): number | undefined {
    return this.widths.get(columnId);
  }

  /** Gets width with fallback to column default. */
  get(columnId: string, defaultWidth: number): number {
    return this.widths.get(columnId) ?? defaultWidth;
  }

  /** Reads a width while enforcing column min/max constraints. */
  getClamped(columnId: string, defaultWidth: number, min: number, max: number): number {
    const [orderedMin, orderedMax] = orderBounds(min, max);
    const width = this.get(columnId, defaultWidth);
    return Math.min(Math.max(orderedMin, width), orderedMax);
  }

  /** Stores a column width. */
  set(columnId: string, width: number): void {
    this.widths.set(columnId, Math.max(0, Math.round(width)));
  }

  /**
   * Stores a width while enforcing constraints. Returns true if the stored value changed.
   */
  setClamped(columnId: string, width: number, min: number, max: number): boolean {
    const [orderedMin, orderedMax] = orderBounds(min, max);
    const clamped = Math.min(Math.max(orderedMin, Math.round(width)), orderedMax);
    if (this.widths.get(columnId) === clamped) {
      return false;
    }
    this.widths.set(columnId, clamped);
    return true;
  }

  /** Removes stored override for a column. */
  remove(columnId: string): boolean {
    return this.widths.delete(columnId);
  }

  /** Clears all stored widths. */
  clear(): void {
    this.widths.clear();
  }

  /** Checks if any widths are stored. */
  isEmpty(): boolean {
    return this.widths.size === 0;
  }

  /** Converts to plain Record object for serialization. */
  toRecord(): Record<string, number> {
    const record: Record<string, number> = {};
    for (const [key, value] of this.widths.entries()) {
      record[key] = value;
    }
    return record;
  }
}

function orderBounds(a: number, b: number): [number, number] {
  return a <= b ? [a, b] : [b, a];
}

/**
 * Headless calculation of resized column width with min/max bounds.
 *
 * Defensive on purpose: pointer coordinates come from browser events and can be
 * non-finite, and bounds may be reversed.
 */
export function calculateResizedWidth(
  initialWidth: number,
  deltaX: number,
  minWidth: number = 40,
  maxWidth: number = 1000
): number {
  const [min, max] = orderBounds(minWidth, maxWidth);
  if (!Number.isFinite(deltaX)) {
    return Math.min(Math.max(min, initialWidth), max);
  }
  const proposed = Math.round(initialWidth + deltaX);
  return Math.min(Math.max(min, proposed), max);
}

/**
 * Steps column width up or down by a discrete increment (e.g. keyboard resize).
 * Supports both Rust-style signed `stepPx` (e.g. `stepResizedWidth(w, -16)`) and
 * boolean `isGrow` for backward compatibility.
 */
export function stepResizedWidth(
  currentWidth: number,
  step: number = KEYBOARD_RESIZE_STEP,
  minWidthOrIsGrow: number | boolean = 40,
  maxWidth: number = 1000,
  legacyMax?: number
): number {
  let delta: number;
  let min = 40;
  let max = 1000;

  if (typeof minWidthOrIsGrow === 'boolean') {
    delta = minWidthOrIsGrow ? Math.abs(step) : -Math.abs(step);
    min = maxWidth;
    max = legacyMax ?? 1000;
  } else {
    delta = step;
    min = minWidthOrIsGrow;
    max = maxWidth;
  }

  return calculateResizedWidth(currentWidth, delta, min, max);
}
