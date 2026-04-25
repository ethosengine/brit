//! Column-packing helper matching git's `column.c` `display_plain` for the
//! `--column=always` / `column.ui = always` defaults: lay items into a
//! width-fitting grid filled column-major.

use std::io::Write;

/// Pack `items` into a column-major grid that fits within `terminal_width`.
/// Per-column width = max(chars().count() of items in that column) + 2 spaces of inter-column padding,
/// except no padding after the last column on each row.
/// Empty input emits nothing.
pub fn write_columns(out: &mut dyn Write, items: &[String], terminal_width: usize) -> std::io::Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let n = items.len();
    let pad = 2usize;

    // Find the largest column count `c` such that the layout fits.
    // Column-major fill: rows = ceil(n / c), column k holds items[k*rows..(k+1)*rows].
    let mut cols = 1usize;
    for c in (1..=n).rev() {
        let rows = n.div_ceil(c);
        let mut total = 0usize;
        let mut fits = true;
        for col in 0..c {
            let start = col * rows;
            let end = (start + rows).min(n);
            if start >= end {
                continue; // empty column — skip
            }
            let widest = items[start..end]
                .iter()
                .map(|s| s.chars().count())
                .max()
                .expect("non-empty slice always has a max");
            // Inter-column padding only between columns (not after the last).
            let inter_pad = if col + 1 == c { 0 } else { pad };
            total = total.saturating_add(widest).saturating_add(inter_pad);
            if total > terminal_width {
                fits = false;
                break;
            }
        }
        if fits {
            cols = c;
            break;
        }
    }

    let rows = n.div_ceil(cols);
    let widths: Vec<usize> = (0..cols)
        .map(|col| {
            let start = col * rows;
            let end = (start + rows).min(n);
            items[start..end].iter().map(|s| s.chars().count()).max().unwrap_or(0)
        })
        .collect();

    for r in 0..rows {
        for (c, _) in widths.iter().enumerate() {
            let i = c * rows + r;
            if i >= n {
                continue;
            }
            let item = &items[i];
            // The last cell on this row is the rightmost column that has a
            // value at row r. Skip padding after it.
            let mut last_on_row = c;
            for c_next in (c + 1)..cols {
                if c_next * rows + r < n {
                    last_on_row = c_next;
                }
            }
            let is_last = c == last_on_row;
            if is_last {
                writeln!(out, "{item}")?;
            } else {
                let used = item.chars().count();
                let target = widths[c] + pad;
                let fill = target.saturating_sub(used);
                write!(out, "{item}{:fill$}", "", fill = fill)?;
            }
        }
    }
    Ok(())
}
