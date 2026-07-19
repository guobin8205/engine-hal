//! Godot Control 的锚点定位系统。
//!
//! 移植自 Godot `scene/gui/control.cpp` 的 `_compute_anchors` + `_compute_offsets`。
//!
//! 核心公式：
//!   x_left   = parent_w * anchor_left   + offset_left
//!   x_right  = parent_w * anchor_right  + offset_right
//!   y_top    = parent_h * anchor_top    + offset_top
//!   y_bottom = parent_h * anchor_bottom + offset_bottom
//!
//! 当 anchor_left == anchor_right → 固定宽度
//! 当 anchor_right > anchor_left → 随父矩形拉伸（响应式布局）

use serde::{Deserialize, Serialize};

/// Godot Control 的锚点配置（4 个 anchor + 4 个 offset）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnchorsPreset {
    pub anchor_left: f32,
    pub anchor_top: f32,
    pub anchor_right: f32,
    pub anchor_bottom: f32,
}

/// Godot Control 的偏移（像素）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Offsets {
    pub offset_left: f32,
    pub offset_top: f32,
    pub offset_right: f32,
    pub offset_bottom: f32,
}

/// Godot 16 个 anchor preset（对应 Godot `LayoutPreset` 枚举）。
///
/// 每个 preset 只是「批量设置 4 个 anchor」的便捷组合。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Preset {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
    CenterLeft,
    CenterTop,
    CenterRight,
    CenterBottom,
    Center,
    LeftWide,
    TopWide,
    RightWide,
    BottomWide,
    VCenterWide,
    HCenterWide,
    FullRect,
}

impl Preset {
    /// 把 preset 转成 4 个 anchor 值。
    pub fn to_anchors(self) -> AnchorsPreset {
        match self {
            Preset::TopLeft => AnchorsPreset { anchor_left: 0.0, anchor_top: 0.0, anchor_right: 0.0, anchor_bottom: 0.0 },
            Preset::TopRight => AnchorsPreset { anchor_left: 1.0, anchor_top: 0.0, anchor_right: 1.0, anchor_bottom: 0.0 },
            Preset::BottomRight => AnchorsPreset { anchor_left: 1.0, anchor_top: 1.0, anchor_right: 1.0, anchor_bottom: 1.0 },
            Preset::BottomLeft => AnchorsPreset { anchor_left: 0.0, anchor_top: 1.0, anchor_right: 0.0, anchor_bottom: 1.0 },
            Preset::CenterLeft => AnchorsPreset { anchor_left: 0.0, anchor_top: 0.5, anchor_right: 0.0, anchor_bottom: 0.5 },
            Preset::CenterTop => AnchorsPreset { anchor_left: 0.5, anchor_top: 0.0, anchor_right: 0.5, anchor_bottom: 0.0 },
            Preset::CenterRight => AnchorsPreset { anchor_left: 1.0, anchor_top: 0.5, anchor_right: 1.0, anchor_bottom: 0.5 },
            Preset::CenterBottom => AnchorsPreset { anchor_left: 0.5, anchor_top: 1.0, anchor_right: 0.5, anchor_bottom: 1.0 },
            Preset::Center => AnchorsPreset { anchor_left: 0.5, anchor_top: 0.5, anchor_right: 0.5, anchor_bottom: 0.5 },
            Preset::LeftWide => AnchorsPreset { anchor_left: 0.0, anchor_top: 0.0, anchor_right: 0.0, anchor_bottom: 1.0 },
            Preset::TopWide => AnchorsPreset { anchor_left: 0.0, anchor_top: 0.0, anchor_right: 1.0, anchor_bottom: 0.0 },
            Preset::RightWide => AnchorsPreset { anchor_left: 1.0, anchor_top: 0.0, anchor_right: 1.0, anchor_bottom: 1.0 },
            Preset::BottomWide => AnchorsPreset { anchor_left: 0.0, anchor_top: 1.0, anchor_right: 1.0, anchor_bottom: 1.0 },
            Preset::VCenterWide => AnchorsPreset { anchor_left: 0.5, anchor_top: 0.0, anchor_right: 0.5, anchor_bottom: 1.0 },
            Preset::HCenterWide => AnchorsPreset { anchor_left: 0.0, anchor_top: 0.5, anchor_right: 1.0, anchor_bottom: 0.5 },
            Preset::FullRect => AnchorsPreset { anchor_left: 0.0, anchor_top: 0.0, anchor_right: 1.0, anchor_bottom: 1.0 },
        }
    }
}

/// 计算结果：最终的 position + size（Godot 坐标系：左上角原点，Y 向下）。
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ComputedRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 根据父矩形大小 + 锚点 + 偏移，计算子节点的最终矩形。
///
/// 这是 Godot Control 布局的核心公式，纯函数，O(1)。
pub fn compute_rect(
    parent_width: f32,
    parent_height: f32,
    anchors: &AnchorsPreset,
    offsets: &Offsets,
) -> ComputedRect {
    let x_left = parent_width * anchors.anchor_left + offsets.offset_left;
    let x_right = parent_width * anchors.anchor_right + offsets.offset_right;
    let y_top = parent_height * anchors.anchor_top + offsets.offset_top;
    let y_bottom = parent_height * anchors.anchor_bottom + offsets.offset_bottom;

    ComputedRect {
        x: x_left,
        y: y_top,
        width: (x_right - x_left).max(0.0),
        height: (y_bottom - y_top).max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 FULL_RECT：子节点填满父矩形
    #[test]
    fn full_rect_fills_parent() {
        let anchors = Preset::FullRect.to_anchors();
        let offsets = Offsets { offset_left: 0.0, offset_top: 0.0, offset_right: 0.0, offset_bottom: 0.0 };
        let rect = compute_rect(960.0, 640.0, &anchors, &offsets);
        assert_eq!(rect, ComputedRect { x: 0.0, y: 0.0, width: 960.0, height: 640.0 });
    }

    /// 测试固定大小 + 左上角对齐（anchor 全 0，offset 定死）
    #[test]
    fn top_left_fixed_size() {
        let anchors = Preset::TopLeft.to_anchors();
        let offsets = Offsets { offset_left: 10.0, offset_top: 20.0, offset_right: 110.0, offset_bottom: 70.0 };
        let rect = compute_rect(960.0, 640.0, &anchors, &offsets);
        assert_eq!(rect, ComputedRect { x: 10.0, y: 20.0, width: 100.0, height: 50.0 });
    }

    /// 测试 CENTER：居中固定大小
    /// Godot 里居中需要 anchor=0.5, offset 为负的半宽/半高
    #[test]
    fn center_fixed_size() {
        let anchors = Preset::Center.to_anchors();
        // 100x50 居中：offset = (-50, -25, 50, 25)
        let offsets = Offsets { offset_left: -50.0, offset_top: -25.0, offset_right: 50.0, offset_bottom: 25.0 };
        let rect = compute_rect(960.0, 640.0, &anchors, &offsets);
        assert_eq!(rect, ComputedRect { x: 430.0, y: 295.0, width: 100.0, height: 50.0 });
        // 960/2 - 50 = 430, 640/2 - 25 = 295
    }

    /// 测试响应式拉伸：BOTTOM_WIDE（底部全宽）
    /// anchor_top=1, anchor_bottom=1, anchor_left=0, anchor_right=1
    /// 高度由 offset 差决定，宽度随父矩形
    #[test]
    fn bottom_wide_stretches_horizontally() {
        let anchors = Preset::BottomWide.to_anchors();
        // 高度 100，离底部 0：offset_top=-100, offset_bottom=0
        let offsets = Offsets { offset_left: 0.0, offset_top: -100.0, offset_right: 0.0, offset_bottom: 0.0 };
        let rect = compute_rect(960.0, 640.0, &anchors, &offsets);
        assert_eq!(rect, ComputedRect { x: 0.0, y: 540.0, width: 960.0, height: 100.0 });
        // y = 640*1 + (-100) = 540, height = 640 - 540 = 100
    }

    /// 测试窗口缩放时锚点行为
    /// 同样的 FullRect，不同父尺寸
    #[test]
    fn full_rect_responds_to_parent_resize() {
        let anchors = Preset::FullRect.to_anchors();
        let offsets = Offsets { offset_left: 0.0, offset_top: 0.0, offset_right: 0.0, offset_bottom: 0.0 };

        let rect_960_640 = compute_rect(960.0, 640.0, &anchors, &offsets);
        assert_eq!(rect_960_640.width, 960.0);

        let rect_1280_720 = compute_rect(1280.0, 720.0, &anchors, &offsets);
        assert_eq!(rect_1280_720.width, 1280.0);
        assert_eq!(rect_1280_720.height, 720.0);
    }

    /// 测试所有 16 个 preset 的 anchor 值
    #[test]
    fn all_presets_produce_valid_anchors() {
        for preset in [
            Preset::TopLeft, Preset::TopRight, Preset::BottomRight, Preset::BottomLeft,
            Preset::CenterLeft, Preset::CenterTop, Preset::CenterRight, Preset::CenterBottom,
            Preset::Center, Preset::LeftWide, Preset::TopWide, Preset::RightWide,
            Preset::BottomWide, Preset::VCenterWide, Preset::HCenterWide, Preset::FullRect,
        ] {
            let a = preset.to_anchors();
            assert!(a.anchor_left >= 0.0 && a.anchor_left <= 1.0, "preset {:?} anchor_left 越界", preset);
            assert!(a.anchor_top >= 0.0 && a.anchor_top <= 1.0, "preset {:?} anchor_top 越界", preset);
            assert!(a.anchor_right >= 0.0 && a.anchor_right <= 1.0, "preset {:?} anchor_right 越界", preset);
            assert!(a.anchor_bottom >= 0.0 && a.anchor_bottom <= 1.0, "preset {:?} anchor_bottom 越界", preset);
        }
    }
}
