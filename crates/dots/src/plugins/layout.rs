//! A small flexbox-wrap grid used by the dashboard.
//!
//! Panes are described as [`Cell`]s — a column `span` and a row-height `weight`.
//! [`grid`] flows them left→right / top→bottom into rows (each row's total span
//! never exceeds the column count), sizes row heights by each row's weight, and
//! sizes the cells within a row by their span. With every cell `span=1
//! weight=1` and `cols=2` this reproduces the original uniform 2-column grid, so
//! built-in panes look unchanged while plugin panes can ask to be wider
//! (`span`) or taller (`weight`).
//!
//! [`move_focus`] does geometric directional navigation over the computed
//! rectangles, so it works for any column count or mix of spans.

use ratatui::layout::Rect;

/// One pane's placement request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Columns spanned (clamped to `1..=cols`).
    pub span: u16,
    /// Row-height weight relative to other rows (clamped to `>= 1`).
    pub weight: u16,
}

impl Cell {
    pub fn new(span: u16, weight: u16) -> Self {
        Self { span: span.max(1), weight: weight.max(1) }
    }
}

/// A run of consecutive cells that share one row.
struct Row {
    start: usize,
    len: usize,
    span_total: u16,
    weight: u16,
}

/// Group cells into rows, greedily wrapping when the next cell would overflow
/// `cols`. A row's weight is the max weight of its cells (the tallest wins).
fn rows(cols: u16, cells: &[Cell]) -> Vec<Row> {
    let cols = cols.max(1);
    let mut out: Vec<Row> = Vec::new();
    for (i, c) in cells.iter().enumerate() {
        let span = c.span.min(cols).max(1);
        match out.last_mut() {
            Some(row) if row.span_total + span <= cols => {
                row.len += 1;
                row.span_total += span;
                row.weight = row.weight.max(c.weight.max(1));
            }
            _ => out.push(Row { start: i, len: 1, span_total: span, weight: c.weight.max(1) }),
        }
    }
    out
}

/// Lay `cells` out inside `area` as a wrapping grid of `cols` columns. Returns
/// one [`Rect`] per cell, in input order. An empty input yields an empty vec.
pub fn grid(area: Rect, cols: u16, cells: &[Cell]) -> Vec<Rect> {
    let cols = cols.max(1);
    let rows = rows(cols, cells);
    if rows.is_empty() {
        return Vec::new();
    }
    let total_weight: u32 = rows.iter().map(|r| r.weight as u32).sum();

    let mut rects = vec![Rect::default(); cells.len()];
    let mut y = area.y;
    let mut weight_acc: u32 = 0;
    let last_row = rows.len() - 1;

    for (ri, row) in rows.iter().enumerate() {
        // Row height ∝ row weight; the final row absorbs the rounding remainder
        // so the grid always reaches the bottom edge.
        weight_acc += row.weight as u32;
        let h = if ri == last_row {
            area.y + area.height - y
        } else {
            let end = (area.height as u32 * weight_acc / total_weight.max(1)) as u16;
            (area.y + end).saturating_sub(y)
        };

        // Cell widths ∝ span; the final cell of the row absorbs the remainder.
        let mut x = area.x;
        let mut span_acc: u16 = 0;
        for k in 0..row.len {
            let idx = row.start + k;
            let span = cells[idx].span.min(cols).max(1);
            span_acc += span;
            let w = if k == row.len - 1 {
                area.x + area.width - x
            } else {
                let end = (area.width as u32 * span_acc as u32 / row.span_total.max(1) as u32) as u16;
                (area.x + end).saturating_sub(x)
            };
            rects[idx] = Rect { x, y, width: w, height: h };
            x += w;
        }
        y += h;
    }
    rects
}

/// Directional focus moves.
#[derive(Clone, Copy)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

fn center(r: Rect) -> (i32, i32) {
    (r.x as i32 + r.width as i32 / 2, r.y as i32 + r.height as i32 / 2)
}

/// The index of the pane reached by moving `dir` from pane `focus`, chosen
/// geometrically over `rects`. Prefers the nearest pane aligned on the cross
/// axis (same row when moving horizontally, same column when moving
/// vertically); stays put when there is nothing in that direction.
pub fn move_focus(rects: &[Rect], focus: usize, dir: Dir) -> usize {
    let Some(&cur) = rects.get(focus) else { return focus };
    let (cx, cy) = center(cur);

    let mut best = focus;
    let mut best_score = i64::MAX;
    for (i, r) in rects.iter().enumerate() {
        if i == focus {
            continue;
        }
        let (x, y) = center(*r);
        let (primary, cross) = match dir {
            Dir::Right => (x - cx, (y - cy).abs()),
            Dir::Left => (cx - x, (y - cy).abs()),
            Dir::Down => (y - cy, (x - cx).abs()),
            Dir::Up => (cy - y, (x - cx).abs()),
        };
        if primary <= 0 {
            continue; // not in the requested direction
        }
        // Cross-axis alignment dominates so we don't jump diagonally; primary
        // distance breaks ties within the same row/column.
        let score = cross as i64 * 100_000 + primary as i64;
        if score < best_score {
            best_score = score;
            best = i;
        }
    }
    best
}

/// Index of the cell covering point `(col, row)` within the laid-out `rects`.
pub fn cell_at(rects: &[Rect], col: u16, row: u16) -> Option<usize> {
    rects.iter().position(|r| col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform(n: usize) -> Vec<Cell> {
        vec![Cell::new(1, 1); n]
    }

    #[test]
    fn uniform_two_col_matches_legacy_grid() {
        // 80×18, 2 columns, 6 uniform cells → 3 rows of two 40×6 cells.
        let area = Rect::new(0, 0, 80, 18);
        let rects = grid(area, 2, &uniform(6));
        assert_eq!(rects[0], Rect::new(0, 0, 40, 6));
        assert_eq!(rects[1], Rect::new(40, 0, 40, 6));
        assert_eq!(rects[2], Rect::new(0, 6, 40, 6));
        assert_eq!(rects[5], Rect::new(40, 12, 40, 6));
    }

    #[test]
    fn full_width_span_fills_its_row() {
        let area = Rect::new(0, 0, 80, 12);
        // one full-width pane, then a normal pair
        let cells = vec![Cell::new(2, 1), Cell::new(1, 1), Cell::new(1, 1)];
        let rects = grid(area, 2, &cells);
        assert_eq!(rects[0], Rect::new(0, 0, 80, 6)); // spans both columns
        assert_eq!(rects[1].width, 40);
        assert_eq!(rects[2].x, 40);
    }

    #[test]
    fn weight_makes_a_row_taller() {
        let area = Rect::new(0, 0, 40, 12);
        // two rows, second weighted 2× → heights 4 and 8.
        let cells = vec![Cell::new(1, 1), Cell::new(1, 2)];
        let rects = grid(area, 1, &cells);
        assert_eq!(rects[0].height, 4);
        assert_eq!(rects[1].height, 8);
        assert_eq!(rects[1].y, 4);
    }

    #[test]
    fn covers_area_exactly() {
        let area = Rect::new(0, 0, 81, 19); // odd sizes to exercise remainders
        let rects = grid(area, 2, &uniform(6));
        // last row/col reach the far edges
        let bottom = rects.iter().map(|r| r.y + r.height).max().unwrap();
        let right = rects.iter().map(|r| r.x + r.width).max().unwrap();
        assert_eq!(bottom, 19);
        assert_eq!(right, 81);
    }

    #[test]
    fn focus_moves_within_uniform_grid() {
        let area = Rect::new(0, 0, 80, 18);
        let r = grid(area, 2, &uniform(6)); // [0 1 / 2 3 / 4 5]
        assert_eq!(move_focus(&r, 0, Dir::Right), 1);
        assert_eq!(move_focus(&r, 1, Dir::Left), 0);
        assert_eq!(move_focus(&r, 0, Dir::Down), 2);
        assert_eq!(move_focus(&r, 2, Dir::Down), 4);
        assert_eq!(move_focus(&r, 4, Dir::Up), 2);
        assert_eq!(move_focus(&r, 1, Dir::Right), 1); // clamped
        assert_eq!(move_focus(&r, 4, Dir::Right), 5);
        assert_eq!(move_focus(&r, 5, Dir::Left), 4);
        assert_eq!(move_focus(&r, 0, Dir::Up), 0);
        assert_eq!(move_focus(&r, 4, Dir::Down), 4);
        assert_eq!(move_focus(&r, 5, Dir::Up), 3);
    }

    #[test]
    fn cell_at_hit_tests() {
        let area = Rect::new(0, 0, 80, 18);
        let r = grid(area, 2, &uniform(6));
        assert_eq!(cell_at(&r, 5, 3), Some(0));
        assert_eq!(cell_at(&r, 45, 3), Some(1));
        assert_eq!(cell_at(&r, 45, 15), Some(5));
        for i in 0..r.len() {
            assert_eq!(cell_at(&r, r[i].x, r[i].y), Some(i));
        }
    }

    #[test]
    fn empty_and_degenerate_inputs() {
        assert!(grid(Rect::new(0, 0, 40, 10), 2, &[]).is_empty());
        // zero cols is treated as one
        let r = grid(Rect::new(0, 0, 40, 10), 0, &uniform(2));
        assert_eq!(r.len(), 2);
    }
}
