//! 布局树：从 .tscn 解析结果构建，递归计算每个节点的最终 position+size。
//!
//! 设计：
//! - 纯计算（无 I/O），便于单元测试
//! - 引擎无关：结果只是 `ComputedLayout`（position + size）
//! - 两层：锚点节点（自算）+ 容器节点（递归布局子节点）

use serde::{Deserialize, Serialize};

use crate::anchor::{compute_rect, AnchorsPreset, ComputedRect, Offsets};

/// 布局节点：从 SceneNode 转换而来，带布局相关信息。
#[derive(Clone, Debug)]
pub struct LayoutNode {
    /// 节点名（调试用）
    pub name: String,
    /// 锚点 + 偏移
    pub anchors: AnchorsPreset,
    pub offsets: Offsets,
    /// 最小尺寸（Container 计算时用）
    pub min_size: Size,
    /// 容器类型（None 表示普通节点，自算布局）
    pub container: Option<ContainerType>,
    /// size_flags（Godot 容器内填充策略）
    pub size_flags_horizontal: SizeFlags,
    pub size_flags_vertical: SizeFlags,
    /// 拉伸比例（EXPAND 时瓜分剩余空间的权重）
    pub stretch_ratio: f32,
    /// layout_mode（0/1=锚点模式，2=容器模式，3=未指定）
    pub layout_mode: i32,
    /// 子节点
    pub children: Vec<LayoutNode>,
    /// 计算结果（布局后填入）
    pub computed: ComputedLayout,
}

/// Godot 的 SizeFlags（bit flag）。
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct SizeFlags {
    pub bits: u32,
}

impl SizeFlags {
    pub const FILL: u32 = 1;
    pub const EXPAND: u32 = 2;
    pub const SHRINK_CENTER: u32 = 4;
    pub const SHRINK_END: u32 = 8;

    pub fn new(bits: u32) -> Self {
        SizeFlags { bits }
    }
    pub fn contains(&self, flag: u32) -> bool {
        self.bits & flag != 0
    }
    pub fn is_fill(&self) -> bool {
        self.contains(Self::FILL)
    }
    pub fn is_expand(&self) -> bool {
        self.contains(Self::EXPAND)
    }
}

/// 尺寸（宽高）。
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Size = Size { width: 0.0, height: 0.0 };
    pub fn new(w: f32, h: f32) -> Self {
        Size { width: w, height: h }
    }
}

/// 容器类型（对应 Godot Container 家族）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ContainerType {
    /// HBoxContainer：水平排列
    HBox { separation: f32 },
    /// VBoxContainer：垂直排列
    VBox { separation: f32 },
    /// MarginContainer：四边留白
    Margin { left: f32, top: f32, right: f32, bottom: f32 },
    /// CenterContainer：子节点居中
    Center,
    /// HSplitContainer：水平分割（split_offset 固定第一个子节点宽度）
    HSplit { separation: f32, split_offset: f32 },
    /// VSplitContainer：垂直分割
    VSplit { separation: f32, split_offset: f32 },
}

/// 节点的计算结果（Godot 坐标系：左上角原点，Y 向下）。
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ComputedLayout {
    /// 相对父节点的位置
    pub position: (f32, f32),
    /// 最终尺寸
    pub size: Size,
}

impl LayoutNode {
    /// 创建一个普通的锚点节点（非容器）。
    pub fn new(name: impl Into<String>, anchors: AnchorsPreset, offsets: Offsets) -> Self {
        LayoutNode {
            name: name.into(),
            anchors,
            offsets,
            min_size: Size::ZERO,
            container: None,
            size_flags_horizontal: SizeFlags::new(SizeFlags::FILL),
            size_flags_vertical: SizeFlags::new(SizeFlags::FILL),
            stretch_ratio: 1.0,
            layout_mode: 0,
            children: Vec::new(),
            computed: ComputedLayout::default(),
        }
    }

    /// 添加子节点。
    pub fn add_child(&mut self, child: LayoutNode) {
        self.children.push(child);
    }

    /// 递归布局：给定父节点尺寸，计算自身 + 所有子节点的最终 position+size。
    ///
    /// 这是布局系统的核心入口。
    pub fn layout(&mut self, parent_size: Size) {
        // layout_mode=2（容器模式）：
        // 如果自己有 anchor/offset（即使 layout_mode=2，Godot 里有时仍有序列化的 offset），
        // 仍然用 anchor 计算（因为父容器会在 layout_container 里覆盖 position）
        // 但如果 anchor 全是 0 且 offset 全是 0，说明确实需要父容器设置
        //
        // 务实做法：layout_mode=2 的节点仍然用 anchor 计算
        // （因为父容器的 layout_container 会覆盖 position/size）
        // 这样非容器父下的 layout_mode=2 节点也能正确定位

        // 用 anchor+offset 计算
        let rect = compute_rect(
            parent_size.width,
            parent_size.height,
            &self.anchors,
            &self.offsets,
        );
        self.computed.position = (rect.x, rect.y);
        self.computed.size = Size::new(rect.width, rect.height);

        if let Some(container) = self.container {
            self.layout_container(container);
        } else {
            let my_size = self.computed.size;
            for child in &mut self.children {
                child.layout(my_size);
            }
        }
    }

    /// 容器布局：覆盖默认锚点行为，按容器规则强制设置子节点位置/尺寸。
    ///
    /// 所有计算出的 size 都会 clamp 到 >= 0（Godot 行为）。
    pub(crate) fn layout_container(&mut self, container: ContainerType) {
        let my_rect = self.computed;
        let my_size = my_rect.size;

        match container {
            ContainerType::Center => {
                if let Some(child) = self.children.first_mut() {
                    let child_min = child.combined_min_size();
                    let cx = (my_size.width - child_min.width) / 2.0;
                    let cy = (my_size.height - child_min.height) / 2.0;
                    child.computed.position = (cx, cy);
                    child.computed.size = child_min;
                    child.layout_children();
                }
            }
            ContainerType::HSplit { separation, split_offset } => {
                // HSplitContainer
                // 当 split_offset > 0：第一个子节点固定 split_offset 宽度
                // 当 split_offset == 0：和 HBox 一样，按 min_size + EXPAND 瓜分
                let n = self.children.len();
                if n == 0 {
                    return;
                }

                if split_offset > 0.0 {
                    // 显式 split_offset
                    let first_w = split_offset;
                    let rest_w = (my_size.width - first_w - separation * (n as f32 - 1.0)).max(0.0);
                    let child = &mut self.children[0];
                    child.computed.position = (0.0, 0.0);
                    child.computed.size = Size::new(first_w, my_size.height);
                    child.layout_children();
                    let mut x = first_w + separation;
                    let rest_per_child = if n > 1 { rest_w / (n as f32 - 1.0) } else { 0.0 };
                    for child in self.children.iter_mut().skip(1) {
                        child.computed.position = (x, 0.0);
                        child.computed.size = Size::new(rest_per_child, my_size.height);
                        child.layout_children();
                        x += rest_per_child + separation;
                    }
                } else {
                    // split_offset=0 → 和 HBox 一样
                    self.layout_container(ContainerType::HBox { separation });
                }
            }
            ContainerType::VSplit { separation, split_offset } => {
                let n = self.children.len();
                if n == 0 {
                    return;
                }

                if split_offset > 0.0 {
                    let first_h = split_offset;
                    let rest_h = (my_size.height - first_h - separation * (n as f32 - 1.0)).max(0.0);
                    let child = &mut self.children[0];
                    child.computed.position = (0.0, 0.0);
                    child.computed.size = Size::new(my_size.width, first_h);
                    child.layout_children();
                    let mut y = first_h + separation;
                    let rest_per_child = if n > 1 { rest_h / (n as f32 - 1.0) } else { 0.0 };
                    for child in self.children.iter_mut().skip(1) {
                        child.computed.position = (0.0, y);
                        child.computed.size = Size::new(my_size.width, rest_per_child);
                        child.layout_children();
                        y += rest_per_child + separation;
                    }
                } else {
                    // split_offset=0 → 和 VBox 一样
                    self.layout_container(ContainerType::VBox { separation });
                }
            }
            ContainerType::Margin { left, top, right, bottom } => {
                // 子节点被裁剪到 margin 内（clamp 防止负值）
                if let Some(child) = self.children.first_mut() {
                    child.computed.position = (left, top);
                    child.computed.size = Size::new(
                        (my_size.width - left - right).max(0.0),
                        (my_size.height - top - bottom).max(0.0),
                    );
                    child.layout_children();
                }
            }
            ContainerType::HBox { separation } => {
                // 水平排列子节点 + size_flags EXPAND 瓜分剩余空间
                let n = self.children.len();
                if n == 0 {
                    return;
                }

                // 预计算：固定宽度子节点的总宽度 + EXPAND 子节点的总 stretch_ratio
                let mut fixed_width = 0.0f32;
                let mut total_stretch = 0.0f32;
                for child in &self.children {
                    let child_min = child.combined_min_size();
                    if child.size_flags_horizontal.is_expand() {
                        total_stretch += child.stretch_ratio;
                    } else {
                        fixed_width += child_min.width;
                    }
                }

                // 预计算：最后一个 EXPAND 子节点的 index
                let last_expand_idx: Option<usize> = self.children.iter().enumerate()
                    .filter(|(_, c)| c.size_flags_horizontal.is_expand())
                    .map(|(i, _)| i)
                    .last();

                let sep_total = separation * (n as f32 - 1.0);
                let avail = (my_size.width - fixed_width - sep_total).max(0.0);

                let mut x = 0.0f32;
                let mut expand_allocated = 0.0f32;
                for (i, child) in self.children.iter_mut().enumerate() {
                    let child_min = child.combined_min_size();
                    let child_width = if child.size_flags_horizontal.is_expand() && total_stretch > 0.0 {
                        let w_expanded = (avail * child.stretch_ratio / total_stretch).floor();
                        // EXPAND 至少拿到 min_size，但不超过容器宽度
                        let w = w_expanded.max(child_min.width).min(my_size.width);
                        expand_allocated += w;
                        if Some(i) == last_expand_idx {
                            (w + (avail - expand_allocated.floor())).max(0.0)
                        } else {
                            w
                        }
                    } else {
                        child_min.width.min(my_size.width)
                    };

                    child.computed.position = (x, 0.0);
                    let child_height = if child.size_flags_vertical.is_fill() {
                        my_size.height
                    } else {
                        child_min.height
                    };
                    child.computed.size = Size::new(child_width, child_height);
                    child.layout_children();
                    x += child_width + separation;
                }
            }
            ContainerType::VBox { separation } => {
                let n = self.children.len();
                if n == 0 {
                    return;
                }

                let mut fixed_height = 0.0f32;
                let mut total_stretch = 0.0f32;
                for child in &self.children {
                    let child_min = child.combined_min_size();
                    if child.size_flags_vertical.is_expand() {
                        total_stretch += child.stretch_ratio;
                    } else {
                        fixed_height += child_min.height;
                    }
                }

                let last_expand_idx: Option<usize> = self.children.iter().enumerate()
                    .filter(|(_, c)| c.size_flags_vertical.is_expand())
                    .map(|(i, _)| i)
                    .last();

                let sep_total = separation * (n as f32 - 1.0);
                let avail = (my_size.height - fixed_height - sep_total).max(0.0);

                let mut y = 0.0f32;
                let mut expand_allocated = 0.0f32;
                for (i, child) in self.children.iter_mut().enumerate() {
                    let child_min = child.combined_min_size();
                    let child_height = if child.size_flags_vertical.is_expand() && total_stretch > 0.0 {
                        let h = (avail * child.stretch_ratio / total_stretch).floor();
                        expand_allocated += h;
                        if Some(i) == last_expand_idx {
                            h + (avail - expand_allocated.floor())
                        } else {
                            h
                        }
                    } else {
                        child_min.height
                    };

                    child.computed.position = (0.0, y);
                    // 交叉轴：FILL 填满，否则用 min_size
                    // Godot 里 FILL（不带 EXPAND）在 VBox 的水平方向也填满
                    let child_width = if child.size_flags_horizontal.is_fill() {
                        my_size.width
                    } else {
                        child_min.width
                    };
                    child.computed.size = Size::new(child_width.max(child_min.width), child_height);
                    child.layout_children();
                    y += child_height + separation;
                }
            }
        }
    }

    /// 递归布局子节点（用自己的当前尺寸作为父尺寸）。
    /// 如果自己是容器，先调 layout_container 给子节点设 position/size。
    fn layout_children(&mut self) {
        if let Some(container) = self.container {
            self.layout_container(container);
        } else {
            let my_size = self.computed.size;
            for child in &mut self.children {
                child.layout(my_size);
            }
        }
    }

    /// 计算节点的最小尺寸（递归）。
    /// 普通节点返回 min_size；容器节点累加子节点最小尺寸。
    pub fn combined_min_size(&self) -> Size {
        if let Some(container) = self.container {
            match container {
                ContainerType::HBox { separation } => {
                    let n = self.children.len();
                    let total: Size = self.children.iter().fold(Size::ZERO, |acc, c| {
                        let cm = c.combined_min_size();
                        Size::new(acc.width + cm.width, acc.height.max(cm.height))
                    });
                    let sep_total = if n > 0 { separation * (n as f32 - 1.0) } else { 0.0 };
                    Size::new(total.width + sep_total, total.height)
                }
                ContainerType::VBox { separation } => {
                    let n = self.children.len();
                    let total: Size = self.children.iter().fold(Size::ZERO, |acc, c| {
                        let cm = c.combined_min_size();
                        Size::new(acc.width.max(cm.width), acc.height + cm.height)
                    });
                    let sep_total = if n > 0 { separation * (n as f32 - 1.0) } else { 0.0 };
                    Size::new(total.width, total.height + sep_total)
                }
                ContainerType::Margin { left, top, right, bottom } => {
                    if let Some(c) = self.children.first() {
                        let cm = c.combined_min_size();
                        Size::new(cm.width + left + right, cm.height + top + bottom)
                    } else {
                        Size::new(left + right, top + bottom)
                    }
                }
                ContainerType::Center => {
                    if let Some(c) = self.children.first() {
                        c.combined_min_size()
                    } else {
                        Size::ZERO
                    }
                }
                ContainerType::HSplit { separation, .. } | ContainerType::VSplit { separation, .. } => {
                    // Split 的 min_size 和 Box 类似
                    let n = self.children.len();
                    let total: Size = self.children.iter().fold(Size::ZERO, |acc, c| {
                        let cm = c.combined_min_size();
                        Size::new(acc.width + cm.width, acc.height.max(cm.height))
                    });
                    let sep_total = if n > 0 { separation * (n as f32 - 1.0) } else { 0.0 };
                    Size::new(total.width + sep_total, total.height)
                }
            }
        } else {
            self.min_size
        }
    }

    /// 收集所有节点（自身 + 子孙）的计算结果，扁平化为 Vec。
    /// 输出**全局坐标**（累加父节点的 position），用于和 Godot 的 get_global_rect() 对比。
    pub fn flatten(&self) -> Vec<FlatNode> {
        let mut out = Vec::new();
        self.flatten_into(&mut out, (0.0, 0.0), &self.name);
        out
    }

    fn flatten_into(&self, out: &mut Vec<FlatNode>, parent_global: (f32, f32), path: &str) {
        let global_pos = (
            parent_global.0 + self.computed.position.0,
            parent_global.1 + self.computed.position.1,
        );
        out.push(FlatNode {
            name: self.name.clone(),
            path: path.to_string(),
            position: global_pos,
            size: self.computed.size,
        });
        for child in &self.children {
            let child_path = format!("{}/{}", path, child.name);
            child.flatten_into(out, global_pos, &child_path);
        }
    }
}

/// 扁平化的布局结果（用于喂给 scene_builder / Cocos）。
#[derive(Clone, Debug, PartialEq)]
pub struct FlatNode {
    pub name: String,
    /// 完整路径（从根开始，如 "MainPanel/HSplitContainer"）
    pub path: String,
    pub position: (f32, f32),
    pub size: Size,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::Preset;

    #[test]
    fn anchor_node_layout() {
        // 一个 FullRect 节点填满 960x640
        let mut node = LayoutNode::new("root", Preset::FullRect.to_anchors(), Offsets {
            offset_left: 0.0, offset_top: 0.0, offset_right: 0.0, offset_bottom: 0.0
        });
        node.layout(Size::new(960.0, 640.0));
        assert_eq!(node.computed.position, (0.0, 0.0));
        assert_eq!(node.computed.size, Size::new(960.0, 640.0));
    }

    #[test]
    fn child_anchor_layout() {
        // 父 FullRect 960x640，子 Center 100x50 居中
        let mut root = LayoutNode::new("root", Preset::FullRect.to_anchors(), Offsets {
            offset_left: 0.0, offset_top: 0.0, offset_right: 0.0, offset_bottom: 0.0
        });
        let mut child = LayoutNode::new("center", Preset::Center.to_anchors(), Offsets {
            offset_left: -50.0, offset_top: -25.0, offset_right: 50.0, offset_bottom: 25.0
        });
        root.add_child(child);
        root.layout(Size::new(960.0, 640.0));

        // 子节点应该居中：(960-100)/2 = 430, (640-50)/2 = 295
        assert_eq!(root.children[0].computed.position, (430.0, 295.0));
        assert_eq!(root.children[0].computed.size, Size::new(100.0, 50.0));
    }

    #[test]
    fn vbox_layout() {
        // VBox 容器里放 3 个固定高度的子节点
        let mut root = LayoutNode::new("vbox", Preset::FullRect.to_anchors(), Offsets {
            offset_left: 0.0, offset_top: 0.0, offset_right: 0.0, offset_bottom: 0.0
        });
        root.container = Some(ContainerType::VBox { separation: 10.0 });

        // 3 个子节点，高度分别为 50/30/40
        for (i, h) in [50.0, 30.0, 40.0].iter().enumerate() {
            let mut c = LayoutNode::new(format!("child{}", i), Preset::TopLeft.to_anchors(), Offsets {
                offset_left: 0.0, offset_top: 0.0, offset_right: 0.0, offset_bottom: -*h
            });
            c.min_size = Size::new(100.0, *h);
            root.add_child(c);
        }

        root.layout(Size::new(960.0, 640.0));

        // 第一个在 y=0，第二个在 y=50+10=60，第三个在 y=60+30+10=100
        assert_eq!(root.children[0].computed.position, (0.0, 0.0));
        assert_eq!(root.children[0].computed.size, Size::new(960.0, 50.0));
        assert_eq!(root.children[1].computed.position, (0.0, 60.0));
        assert_eq!(root.children[1].computed.size, Size::new(960.0, 30.0));
        assert_eq!(root.children[2].computed.position, (0.0, 100.0));
        assert_eq!(root.children[2].computed.size, Size::new(960.0, 40.0));
    }

    #[test]
    fn margin_container_layout() {
        // Margin 容器：margin 10/20/30/40
        let mut root = LayoutNode::new("margin", Preset::FullRect.to_anchors(), Offsets {
            offset_left: 0.0, offset_top: 0.0, offset_right: 0.0, offset_bottom: 0.0
        });
        root.container = Some(ContainerType::Margin { left: 10.0, top: 20.0, right: 30.0, bottom: 40.0 });
        root.add_child(LayoutNode::new("content", Preset::FullRect.to_anchors(), Offsets {
            offset_left: 0.0, offset_top: 0.0, offset_right: 0.0, offset_bottom: 0.0
        }));

        root.layout(Size::new(960.0, 640.0));

        // 子节点应该被裁剪到 margin 内
        assert_eq!(root.children[0].computed.position, (10.0, 20.0));
        assert_eq!(root.children[0].computed.size, Size::new(960.0 - 10.0 - 30.0, 640.0 - 20.0 - 40.0));
    }

    #[test]
    fn flatten_collects_all_nodes() {
        let mut root = LayoutNode::new("root", Preset::FullRect.to_anchors(), Offsets {
            offset_left: 0.0, offset_top: 0.0, offset_right: 0.0, offset_bottom: 0.0
        });
        root.add_child(LayoutNode::new("c1", Preset::Center.to_anchors(), Offsets {
            offset_left: -50.0, offset_top: -25.0, offset_right: 50.0, offset_bottom: 25.0
        }));
        root.layout(Size::new(960.0, 640.0));

        let flat = root.flatten();
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].name, "root");
        assert_eq!(flat[1].name, "c1");
    }
}
