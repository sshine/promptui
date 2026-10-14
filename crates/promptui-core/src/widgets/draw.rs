use ratatui::buffer::Buffer;
use ratatui::style::Style;

/// Write `text` into `buf` starting at `(x, y)`, one cell per character, clipped
/// at `x_max` (exclusive). Returns the column after the last cell written, so
/// callers can continue a run of differently styled text.
pub fn draw_text(buf: &mut Buffer, x: u16, y: u16, x_max: u16, text: &str, style: Style) -> u16 {
    let mut end = x;
    for (cx, ch) in (x..x_max).zip(text.chars()) {
        if let Some(cell) = buf.cell_mut((cx, y)) {
            cell.set_char(ch);
            cell.set_style(style);
        }
        end = cx + 1;
    }
    end
}
