/** Top padding of the board list. Matches the padding set on the grid. */
export const BOARD_PAD = 16;
/** Space under each card row. Matches the row's bottom margin. */
export const BOARD_GAP = 12;

/**
 * Cell range to keep mounted for a board scroll position.
 * Two rows stay mounted above and below the viewport, and the pitch includes
 * the gap, so a row is not removed while it is still on screen.
 */
export function boardSpan(offset: number, viewport: number, cardH: number, columns: number, count: number) {
  const cols = Math.max(1, columns);
  const stride = Math.max(1, cardH) + BOARD_GAP;
  const top = Math.max(0, offset);
  const view = Math.max(1, viewport);
  const first = Math.max(0, Math.floor((top - BOARD_PAD) / stride));
  const last = Math.max(first, Math.floor((top + view - BOARD_PAD) / stride));
  const start = Math.max(0, first - 2) * cols;
  const end = Math.min(Math.max(0, count), (last + 3) * cols);
  return { start, end: Math.max(start, end) };
}
