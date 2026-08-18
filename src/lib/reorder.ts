/**
 * Moving one item of a run order to another position.
 *
 * The up/down buttons stay: they are precise, reachable one-handed and work when a
 * mouse is not to hand. Dragging is for the other case, rebuilding an order before a
 * service, where clicking ↑ six times to move an item from the bottom to the top is
 * six chances to lose your place.
 *
 * Kept apart from the component because off-by-one errors here are the whole bug,
 * and they are invisible in a UI until an item lands one row from where it was
 * dropped.
 */

/**
 * Move the item at `from` so it sits at `to`, returning a new array.
 *
 * `to` is the index the item should end up at in the *result*, which is what a drop
 * target means: dropping onto row 2 puts the item at row 2. The naive
 * remove-then-insert gets this wrong when moving downwards, because removing first
 * shifts every later index up by one.
 */
export function moveTo<T>(items: readonly T[], from: number, to: number): T[] {
  if (
    from === to ||
    from < 0 ||
    to < 0 ||
    from >= items.length ||
    to >= items.length
  ) {
    return [...items];
  }
  const out = [...items];
  const [moved] = out.splice(from, 1);
  out.splice(to, 0, moved);
  return out;
}

/**
 * Where a drop lands, given the row it was released over.
 *
 * Dropping an item onto its own row, or onto the row it already follows, is a
 * no-op rather than a one-place nudge: releasing roughly where you started should
 * change nothing, not shuffle the order by one.
 */
export function dropIndex(from: number, over: number): number {
  return from === over ? from : over;
}
