//! hal-font: 字体度量（对齐 Godot 默认字体 OpenSans SemiBold）。
//!
//! Godot 4.x 默认字体是 OpenSans SemiBold（woff2 格式，嵌入引擎）。
//! hal-layout 算叶子节点 min_size 时需要文字宽度/高度，用同一字体度量能得到和 Godot 一致的结果。
//!
//! 本 crate 只做度量（glyph advance + 行高），不做渲染。

use fontdue::Font;
use std::sync::OnceLock;

/// Godot 默认字体 OpenSans SemiBold（TTF 格式，150KB，从 woff2 转换）。
const OPENSAN_TTF: &[u8] = include_bytes!("../assets/OpenSans_SemiBold.ttf");

static FONT: OnceLock<Font> = OnceLock::new();

fn font() -> &'static Font {
    FONT.get_or_init(|| {
        Font::from_bytes(OPENSAN_TTF.to_vec(), fontdue::FontSettings::default())
            .expect("解析 OpenSans 失败")
    })
}

/// 从 TTF 的 OS/2 表读 usWinAscent / usWinDescent（fontdue 不暴露，需自己解析）。
///
/// **关键**：Godot 的 Font::get_height 实际用的是 FreeType 的
/// `face->size->metrics.ascender/descender`，而 FreeType 对 OpenSans 这类
/// 设了 USE_TYPO_METRICS 的字体...实测 Godot Label@16 = 23，
/// 正好等于 OS/2 的 usWinAscent(2302) + usWinDescent(651) 在 @16 的缩放值 23.07。
/// fontdue 的 horizontal_line_metrics 用 hhea 表（2189/-600 → 21.79），不准。
struct WinMetrics {
    win_ascent: u16,
    win_descent: u16,
    units_per_em: u16,
}

static WIN_METRICS: OnceLock<Option<WinMetrics>> = OnceLock::new();

fn win_metrics() -> Option<&'static WinMetrics> {
    WIN_METRICS
        .get_or_init(|| parse_os2_win_metrics(OPENSAN_TTF))
        .as_ref()
}

/// 解析 TTF 的 OS/2 表读 usWinAscent(偏移 74) / usWinDescent(偏移 76)，
/// 以及 head 表的 unitsPerEm(偏移 18)。
fn parse_os2_win_metrics(data: &[u8]) -> Option<WinMetrics> {
    if data.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([data[4], data[5]]);
    let mut os2_offset = None;
    let mut head_offset = None;
    for i in 0..num_tables as usize {
        let entry_off = 12 + i * 16;
        if entry_off + 16 > data.len() {
            break;
        }
        let tag = &data[entry_off..entry_off + 4];
        let offset = u32::from_be_bytes([
            data[entry_off + 8],
            data[entry_off + 9],
            data[entry_off + 10],
            data[entry_off + 11],
        ]) as usize;
        if tag == b"OS/2" {
            os2_offset = Some(offset);
        } else if tag == b"head" {
            head_offset = Some(offset);
        }
    }

    let os2_off = os2_offset?;
    if os2_off + 78 > data.len() {
        return None;
    }
    // OS/2 表布局：usWinAscent @ offset 74, usWinDescent @ offset 76
    let win_ascent = u16::from_be_bytes([data[os2_off + 74], data[os2_off + 75]]);
    let win_descent = u16::from_be_bytes([data[os2_off + 76], data[os2_off + 77]]);

    let head_off = head_offset?;
    if head_off + 20 > data.len() {
        return None;
    }
    let units_per_em = u16::from_be_bytes([data[head_off + 18], data[head_off + 19]]);

    Some(WinMetrics {
        win_ascent,
        win_descent,
        units_per_em,
    })
}

/// 度量文字在指定字号下的像素宽度（advance width 之和）。
///
/// 对齐 Godot 的文字渲染宽度（fontdue glyph advance，和 FreeType 一致）。
pub fn text_width(text: &str, font_size: f32) -> f32 {
    let f = font();
    let mut width = 0.0f32;
    for ch in text.chars() {
        let metrics = f.metrics(ch, font_size);
        width += metrics.advance_width;
    }
    width
}

/// 度量文字在指定字号下的最大行宽（多行 text 按 \n 分割，取最长行）。
pub fn text_max_line_width(text: &str, font_size: f32) -> f32 {
    text.split('\n')
        .map(|line| text_width(line, font_size))
        .fold(0.0f32, f32::max)
}

/// 度量指定字号下的单行高度（对齐 Godot `Font::get_height`）。
///
/// Godot 的 get_height 用 FreeType 的 ascender+descender，对 OpenSans 这类字体
/// 等价于 OS/2 表的 usWinAscent + usWinDescent 按 font_size/unitsPerEm 缩放。
/// fontdue 的 horizontal_line_metrics 用 hhea 表（偏小），这里改用 OS/2 win metrics。
pub fn line_height(font_size: f32) -> f32 {
    if let Some(wm) = win_metrics() {
        let scale = font_size / wm.units_per_em as f32;
        return (wm.win_ascent as f32 + wm.win_descent as f32) * scale;
    }
    // fallback：fontdue hhea（偏小 ~1.2px）
    let f = font();
    f.horizontal_line_metrics(font_size)
        .map(|lm| lm.ascent - lm.descent)
        .unwrap_or(font_size * 1.25)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_font() {
        let _ = font();
    }

    #[test]
    fn ascii_width() {
        // "HSlider" @16, Godot min_width = 56
        let w = text_width("HSlider", 16.0);
        eprintln!("HSlider @16 = {:.2} (godot 56)", w);
        assert!(w > 50.0 && w < 60.0, "HSlider 宽度异常: {}", w);
    }

    #[test]
    fn line_height_matches_godot() {
        // Godot Label@16 实测 min_height = 23
        // OS/2 win: (2302+651)*16/2048 = 23.07
        let h16 = line_height(16.0);
        let h24 = line_height(24.0);
        eprintln!("line_height @16 = {:.3} (godot 23), @24 = {:.3} (godot ~34)", h16, h24);
        assert!((h16 - 23.0).abs() < 0.5, "@16 行高应接近 23，实际 {}", h16);
        assert!((h24 - 34.0).abs() < 0.8, "@24 行高应接近 34，实际 {}", h24);
    }

    #[test]
    fn linkbutton_width() {
        let w = text_width("LinkButton (hover me for tooltip)", 16.0);
        eprintln!("LinkButton text @16 = {:.2} (godot 256)", w);
        assert!(w > 250.0 && w < 260.0, "LinkButton 文字宽度异常: {}", w);
    }

    #[test]
    fn title_font24() {
        let w = text_width("Numbers", 24.0);
        eprintln!("Numbers @24 = {:.2} (godot 108)", w);
        assert!(w > 105.0 && w < 112.0, "Numbers 宽度异常: {}", w);
    }
}
