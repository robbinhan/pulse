import { expect, test } from "nsh/test";
import { BOARD_GAP, BOARD_PAD, boardSpan } from "./board-span";

test("a row still on screen stays mounted", () => {
  const cardH = 450;
  const stride = cardH + BOARD_GAP;
  // Old math used cardH alone, so offset 900 looked like row 2 and dropped row 1.
  // Row 1 still occupies the viewport: it ends 28px below this offset.
  const offset = 900;
  const row1End = BOARD_PAD + stride + cardH;
  expect(row1End).toBeGreaterThan(offset);
  const span = boardSpan(offset, 900, cardH, 2, 40);
  expect(span.start).toBe(0);
  expect(span.end).toBeGreaterThan(4);
});

test("the range grows before the next row enters", () => {
  const cardH = 450;
  const stride = cardH + BOARD_GAP;
  const row2Start = BOARD_PAD + stride * 2;
  const span = boardSpan(row2Start - 40, 800, cardH, 2, 40);
  expect(span.end).toBeGreaterThanOrEqual(6);
});
