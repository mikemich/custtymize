//! Digital rain. Purely decorative; any key returns to work.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::theme::Theme;
use crate::app::MatrixState;

const GLYPHS: &[char] = &[
    'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ', 'ﾀ', 'ﾁ', 'ﾂ', 'ﾃ', 'ﾄ', 'ﾅ', 'ﾆ',
    'ﾇ', 'ﾈ', 'ﾉ', 'ﾊ', 'ﾋ', 'ﾌ', 'ﾍ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ', 'ﾔ', 'ﾕ', 'ﾖ', 'ﾗ', 'ﾘ', 'ﾙ', 'ﾚ', 'ﾛ', 'ﾜ',
    '0', '1', '2', '3', '4', '5', '7', '8', '9', 'Z', ':', '=', '*', '+', '<', '>', '¦', '$',
];

pub fn draw(f: &mut Frame, area: Rect, m: &MatrixState, t: &Theme) {
    let green = match t.ok {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0x50, 0xfa, 0x7b),
    };
    let frame_no = (m.started.elapsed().as_millis() / 60) as usize;
    let buf = f.buffer_mut();
    for x in area.left()..area.right() {
        let col = (x - area.left()) as usize;
        let Some(&(head, _, len)) = m.drops.get(col) else { continue };
        let head = head % (area.height as f32 + len as f32 + 20.0);
        for k in 0..len {
            let y = head as i32 - k as i32;
            if y < 0 || y >= area.height as i32 {
                continue;
            }
            let fade = 1.0 - k as f32 / len as f32;
            let seed = col * 31 + y as usize * 17 + if k == 0 { frame_no } else { frame_no / 7 };
            let ch = GLYPHS[seed % GLYPHS.len()];
            let style = if k == 0 {
                Style::new().fg(Color::Rgb(0xe8, 0xff, 0xe8)).add_modifier(Modifier::BOLD)
            } else {
                let s = |c: u8| (c as f32 * fade) as u8;
                Style::new().fg(Color::Rgb(s(green.0), s(green.1), s(green.2)))
            };
            if let Some(cell) = buf.cell_mut((x, area.top() + y as u16)) {
                cell.set_char(ch).set_style(style);
            }
        }
    }
    let msg = " follow the white rabbit · any key ";
    let w = msg.chars().count() as u16;
    if area.width > w {
        let r = Rect::new(area.x + (area.width - w) / 2, area.bottom() - 1, w, 1);
        buf.set_string(r.x, r.y, msg, Style::new().fg(t.bg).bg(t.ok));
    }
}
