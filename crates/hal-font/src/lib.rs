//! hal-font: 字体度量（对齐 Godot 默认字体 OpenSans SemiBold）。
//!
//! Godot 4.x 默认字体是 OpenSans SemiBold（woff2 格式，嵌入引擎）。
//! hal-layout 算叶子节点 min_size 时需要文字宽度，用同一字体度量能得到和 Godot 一致的结果。
//!
//! 本 crate 只做度量（glyph advance width），不做渲染。

use fontdue::Font;
use std::sync::OnceLock;

/// Godot 默认字体 OpenSans SemiBold（TTF 格式，150KB，从 woff2 转换）。
/// 用 include_bytes! 嵌入二进制，无需运行时文件依赖。
const OPENSAN_TTF: &[u8] = include_bytes!("../assets/OpenSans_SemiBold.ttf");

static FONT: OnceLock<Font> = OnceLock::new();

/// 获取全局 Font 实例（首次调用时解析）。
fn font() -> &'static Font {
    FONT.get_or_init(|| {
        Font::from_bytes(OPENSAN_TTF, fontdue::FontSettings::default())
            .expect("解析 OpenSans 失败")
    })
}

/// 度量文字在指定字号下的像素宽度（advance width 之和）。
///
/// 对齐 Godot 的文字渲染宽度（不含 kerning，Godot 默认也不开 kerning）。
/// 返回的宽度单位是像素（font_size 像素高度下的水平 advance）。
pub fn text_width(text: &str, font_size: f32) -> f32 {
    let f = font();
    let mut width = 0.0f32;
    for ch in text.chars() {
        // fontdue 的 metrics 返回 Metrics 结构体，advance_width 是字符前进宽度
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

/// 度量指定字号下的单行高度（对齐 Godot Font::get_height）。
///
/// Godot 的 get_height = ascent + descent（不含 line_gap），
/// 数据来自 TTF 的 hhea/OS_2 表，fontdue 解析同样的表。
/// fontdue 的 horizontal_line_metrics：ascent 为正、descent 为负、line_gap 为正，
/// 这里只用 ascent - descent（即 ascent + |descent|），和 Godot 一致。
pub fn line_height(font_size: f32) -> f32 {
    let f = font();
    match f.horizontal_line_metrics(font_size) {
        Some(lm) => lm.ascent - lm.descent, // descent 为负，相减即加绝对值
        None => font_size * 1.25,            // fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_font() {
        // 确认字体能加载
        let _ = font();
    }

    #[test]
    fn ascii_width() {
        // "HSlider" 在 font_size=16 下 Godot 实测 min_width = 56
        // 56/7 chars ≈ 8.0/char
        let w = text_width("HSlider", 16.0);
        eprintln!("HSlider @16 = {:.1} (godot 56)", w);
        assert!(w > 40.0 && w < 80.0, "HSlider 宽度异常: {}", w);
    }

    #[test]
    fn line_height_check() {
        // Godot font_size=16 的 Label 行高约 20-23px（ascent+descent）
        let h16 = line_height(16.0);
        eprintln!("line_height @16 = {:.2} (godot Label ~20-23)", h16);
        assert!(h16 > 15.0 && h16 < 30.0, "行高异常: {}", h16);
    }

    #[test]
    fn linkbutton_width() {
        // "LinkButton (hover me for tooltip)" 33 chars, Godot min_width = 256
        let w = text_width("LinkButton (hover me for tooltip)", 16.0);
        eprintln!("LinkButton text @16 = {:.1} (godot text part, button total 256)", w);
        // 文字部分应接近 256 - padding
        assert!(w > 200.0 && w < 280.0, "LinkButton 文字宽度异常: {}", w);
    }

    #[test]
    fn title_font24() {
        // Title 用 font_size=24, "Numbers" 7 chars, Godot min_width = 108
        let w = text_width("Numbers", 24.0);
        eprintln!("Numbers @24 = {:.1} (godot 108)", w);
        assert!(w > 90.0 && w < 130.0, "Numbers 宽度异常: {}", w);
    }
}
