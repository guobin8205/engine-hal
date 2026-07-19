//! 布局树：从 .tscn 解析结果构建，递归计算每个节点的最终 position+size。
//!
//! 设计：
//! - 纯计算（无 I/O），便于单元测试
//! - 引擎无关：结果只是 `ComputedLayout`（position + size）
//! - 两层：锚点节点（自算）+ 容器节点（递归布局子节点）

use serde::{Deserialize, Serialize};

use crate::anchor::{compute_rect, AnchorsPreset, ComputedRect, Offsets};

/// grow_direction（Godot Control 的扩展方向）
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum GrowDirection {
    #[default]
    Begin,
    End,
    Both,
}

impl GrowDirection {
    pub fn from_int(v: i64) -> Self {
        match v {
            0 => GrowDirection::Begin,
            1 => GrowDirection::End,
            _ => GrowDirection::Both,
        }
    }
}

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
    /// grow_direction（当 min_size > anchor 矩形时扩展方向）
    pub grow_h: GrowDirection,
    pub grow_v: GrowDirection,
    /// 是否可见（hidden 节点在容器布局中跳过）
    pub visible: bool,
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
    /// TabContainer：只渲染当前 tab（current_tab），顶部留出 tab_bar_height。
    /// content 侧用 panel_margins 作为内边距（默认主题 panel_style margin 通常为 0）。
    Tab { tab_bar_height: f32, current_tab: i32, panel_left: f32, panel_top: f32, panel_right: f32, panel_bottom: f32 },
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
            grow_h: GrowDirection::default(),
            grow_v: GrowDirection::default(),
            visible: true,
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

        // grow_direction: 如果 min_size > anchor 算出的 size，扩展
        // layout_mode=2（容器子节点）不应用 grow（size 由父容器决定）
        if self.layout_mode != 2 {
        let min = self.combined_min_size();
        if min.width > self.computed.size.width {
            let diff = min.width - self.computed.size.width;
            match self.grow_h {
                GrowDirection::Both => {
                    // 居中扩展
                    self.computed.position.0 -= diff * 0.5;
                    self.computed.size.width = min.width;
                }
                GrowDirection::End => {
                    self.computed.size.width = min.width;
                }
                GrowDirection::Begin => {
                    self.computed.position.0 -= diff;
                    self.computed.size.width = min.width;
                }
            }
        }
        if min.height > self.computed.size.height {
            let diff = min.height - self.computed.size.height;
            match self.grow_v {
                GrowDirection::Both => {
                    self.computed.position.1 -= diff * 0.5;
                    self.computed.size.height = min.height;
                }
                GrowDirection::End => {
                    self.computed.size.height = min.height;
                }
                GrowDirection::Begin => {
                    self.computed.position.1 -= diff;
                    self.computed.size.height = min.height;
                }
            }
        }
        } // end if layout_mode != 2

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
                // SplitContainer 算法精确移植 Godot 4.6：
                //   split_container.cpp::_update_default_dragger_positions
                //   + _update_dragger_positions + _resort
                // Godot 4.6 默认 DISABLE_DEPRECATED，所以不走 2-EXPAND 特殊情况。
                let n = self.children.len();
                if n == 0 { return; }
                if n == 1 {
                    self.children[0].computed.position = (0.0, 0.0);
                    self.children[0].computed.size = my_size;
                    self.children[0].layout_children();
                    return;
                }

                let min_sizes: Vec<f32> = self.children.iter()
                    .map(|c| c.combined_min_size().width).collect();
                let is_expand: Vec<bool> = self.children.iter()
                    .map(|c| c.size_flags_horizontal.is_expand() && c.stretch_ratio > 0.0)
                    .collect();
                let stretch_ratios: Vec<f32> = self.children.iter()
                    .map(|c| c.stretch_ratio).collect();

                let final_sizes = compute_split_sizes(
                    my_size.width, separation, &min_sizes, &is_expand, &stretch_ratios,
                    split_offset,
                );

                let mut x = 0.0f32;
                for (i, child) in self.children.iter_mut().enumerate() {
                    let w = final_sizes[i].max(0.0);
                    child.computed.position = (x, 0.0);
                    child.computed.size = Size::new(w, my_size.height);
                    child.layout_children();
                    x += w + separation;
                }
            }
            ContainerType::VSplit { separation, split_offset } => {
                // 同 HSplit，只是轴向换成垂直
                let n = self.children.len();
                if n == 0 { return; }
                if n == 1 {
                    self.children[0].computed.position = (0.0, 0.0);
                    self.children[0].computed.size = my_size;
                    self.children[0].layout_children();
                    return;
                }

                let min_sizes: Vec<f32> = self.children.iter()
                    .map(|c| c.combined_min_size().height).collect();
                let is_expand: Vec<bool> = self.children.iter()
                    .map(|c| c.size_flags_vertical.is_expand() && c.stretch_ratio > 0.0)
                    .collect();
                let stretch_ratios: Vec<f32> = self.children.iter()
                    .map(|c| c.stretch_ratio).collect();

                let final_sizes = compute_split_sizes(
                    my_size.height, separation, &min_sizes, &is_expand, &stretch_ratios,
                    split_offset,
                );

                let mut y = 0.0f32;
                for (i, child) in self.children.iter_mut().enumerate() {
                    let h = final_sizes[i].max(0.0);
                    child.computed.position = (0.0, y);
                    child.computed.size = Size::new(my_size.width, h);
                    child.layout_children();
                    y += h + separation;
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
            ContainerType::Tab { tab_bar_height, current_tab, panel_left, panel_top, panel_right, panel_bottom } => {
                // 对齐 Godot TabContainer::_repaint_internal：
                // 当前 tab（current_tab）用 FullRect + 顶部偏移 tab_bar_height，
                // 再叠加 panel_style 的四边 margin。其余 tab 隐藏（不参与布局）。
                let idx = current_tab as usize;
                if idx < self.children.len() {
                    let child = &mut self.children[idx];
                    let left = panel_left;
                    let top = tab_bar_height + panel_top;
                    let right = panel_right;
                    let bottom = panel_bottom;
                    child.computed.position = (left, top);
                    child.computed.size = Size::new(
                        (my_size.width - left - right).max(0.0),
                        (my_size.height - top - bottom).max(0.0),
                    );
                    child.layout_children();
                }
            }
            ContainerType::HBox { separation } => {
                let n = self.children.len();
                if n == 0 {
                    return;
                }

                // 统计 EXPAND 子节点
                let expand_count = self.children.iter()
                    .filter(|c| c.size_flags_horizontal.is_expand()).count();
                let total_stretch: f32 = self.children.iter()
                    .filter(|c| c.size_flags_horizontal.is_expand())
                    .map(|c| c.stretch_ratio).sum();

                // Godot 特殊情况：恰好 2 个 EXPAND 子节点时，忽略 min_size，直接按 ratio 瓜分总宽度
                if expand_count == 2 && n == 2 && total_stretch > 0.0 {
                    let sep_total = separation * (n as f32 - 1.0);
                    let total_w = my_size.width;
                    let ratio0 = self.children[0].stretch_ratio / total_stretch;
                    let first_w = (total_w * ratio0 - separation * 0.5).floor();
                    let second_w = total_w - first_w - sep_total;

                    self.children[0].computed.position = (0.0, 0.0);
                    self.children[0].computed.size = Size::new(first_w, my_size.height);
                    self.children[0].layout_children();

                    self.children[1].computed.position = (first_w + separation, 0.0);
                    self.children[1].computed.size = Size::new(second_w, my_size.height);
                    self.children[1].layout_children();
                    return;
                }

                // 常规 HBox 算法
                // 非 EXPAND 子节点的 min_size 计入 fixed_width
                // EXPAND 子节点瓜分剩余空间（avail = container_w - fixed - sep）
                let mut fixed_width = 0.0f32;
                for child in &self.children {
                    if !child.size_flags_horizontal.is_expand() {
                        fixed_width += child.combined_min_size().width;
                    }
                }

                let last_expand_idx: Option<usize> = self.children.iter().enumerate()
                    .filter(|(_, c)| c.size_flags_horizontal.is_expand())
                    .map(|(i, _)| i)
                    .last();

                let sep_total = separation * (n as f32 - 1.0);
                let avail = my_size.width - fixed_width - sep_total;

                let mut x = 0.0f32;
                let mut expand_allocated = 0.0f32;
                for (i, child) in self.children.iter_mut().enumerate() {
                    let child_min = child.combined_min_size();
                    let child_width = if child.size_flags_horizontal.is_expand() && total_stretch > 0.0 && avail > 0.0 {
                        let w = (avail * child.stretch_ratio / total_stretch).floor();
                        expand_allocated += w;
                        if Some(i) == last_expand_idx {
                            w + (avail - expand_allocated.floor())
                        } else {
                            w
                        }
                    } else {
                        child_min.width
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
    /// 如果 min_size 被显式设过（非 0），优先用它（inject 模式）。
    pub fn combined_min_size(&self) -> Size {
        // 如果有显式 min_size（非 0），直接用（inject 模式或 custom_minimum_size）
        if self.min_size.width > 0.0 || self.min_size.height > 0.0 {
            return self.min_size;
        }

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
                ContainerType::Tab { tab_bar_height, current_tab, panel_left, panel_top, panel_right, panel_bottom } => {
                    // min_size = 当前 tab 内容 min_size + tab_bar_height + panel margins
                    let idx = current_tab as usize;
                    if idx < self.children.len() {
                        let cm = self.children[idx].combined_min_size();
                        Size::new(
                            cm.width + panel_left + panel_right,
                            cm.height + tab_bar_height + panel_top + panel_bottom,
                        )
                    } else {
                        Size::new(0.0, tab_bar_height)
                    }
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

    /// 收集所有节点，输出**局部坐标**（相对父节点的 position），用于和 Godot 的 position 属性对比。
    pub fn flatten_local(&self) -> Vec<FlatNode> {
        let mut out = Vec::new();
        self.flatten_local_into(&mut out, &self.name);
        out
    }

    fn flatten_local_into(&self, out: &mut Vec<FlatNode>, path: &str) {
        out.push(FlatNode {
            name: self.name.clone(),
            path: path.to_string(),
            position: self.computed.position, // 局部坐标（相对父节点）
            size: self.computed.size,
        });
        for child in &self.children {
            let child_path = format!("{}/{}", path, child.name);
            child.flatten_local_into(out, &child_path);
        }
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

/// SplitContainer 核心算法（移植自 Godot 4.6 `split_container.cpp`）。
///
/// 输入沿主轴的信息，返回每个子节点的最终尺寸（不含 separation）。
/// 调用方负责按 `final_size[i] + separation` 累加 position。
///
/// 算法分两步（对齐 Godot `_update_default_dragger_positions` + `_update_dragger_positions`）：
/// 1. 用 stretch 算法算出每个子节点的"默认尺寸" → 推出 `default_dragger_positions`
/// 2. `dragger_positions[i] = CLAMP(default[i] + split_offset, valid_range)`
///    其中 valid_range 由左右子节点 min_size 决定（防止拖拽越过 min 边界）
///
/// Godot 4.6 默认 `DISABLE_DEPRECATED`，所以不走"2 个 EXPAND 特殊情况"。
fn compute_split_sizes(
    size: f32,
    separation: f32,
    min_sizes: &[f32],
    is_expand: &[bool],
    stretch_ratios: &[f32],
    split_offset: f32,
) -> Vec<f32> {
    let n = min_sizes.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![size.max(0.0)];
    }

    let sep = separation;
    let size_int = size.round() as i64;

    // ---- Step 1: stretch 算法（_update_default_dragger_positions 的 StretchData 部分）----
    #[derive(Clone, Copy)]
    struct StretchData {
        min_size: i64,
        final_size: i64,
        stretch_ratio: f32,
        expand_flag: bool,
        will_stretch: bool,
    }

    let mut stretch_data: Vec<StretchData> = (0..n)
        .map(|i| StretchData {
            min_size: min_sizes[i].round() as i64,
            final_size: min_sizes[i].round() as i64,
            stretch_ratio: if is_expand[i] { stretch_ratios[i] } else { 0.0 },
            expand_flag: is_expand[i],
            will_stretch: is_expand[i],
        })
        .collect();

    // stretchable_space = size - sep * (n - 1)
    let mut stretchable_space = size_int - sep.round() as i64 * (n as i64 - 1);
    let mut stretch_total: f32 = 0.0;
    let mut expand_count = 0;
    for sdata in &stretch_data {
        if sdata.expand_flag {
            stretch_total += sdata.stretch_ratio;
            expand_count += 1;
        } else {
            stretchable_space -= sdata.min_size;
        }
    }

    // while 循环（Godot 第 765-804 行）。max_size 限制不实现（默认无 max）。
    while stretch_total > 0.0 && stretchable_space > 0 {
        let mut refit_successful = true;
        let mut error: f32 = 0.0;
        for sdata in &mut stretch_data {
            if !sdata.will_stretch {
                continue;
            }
            let desired = sdata.stretch_ratio / stretch_total * stretchable_space as f32;
            error += desired - desired.trunc();
            if (desired as i64) < sdata.min_size {
                // 不再 stretch，扣回 min_size
                stretch_total -= sdata.stretch_ratio;
                stretchable_space -= sdata.min_size;
                sdata.will_stretch = false;
                sdata.final_size = sdata.min_size;
                refit_successful = false;
                break;
            } else {
                sdata.final_size = desired as i64;
                if error >= 1.0 {
                    sdata.final_size += 1;
                    error -= 1.0;
                }
            }
        }
        if refit_successful {
            break;
        }
    }

    // default_dragger_positions（Godot 第 806-824 行）
    let mut default_positions: Vec<i64> = Vec::with_capacity(n - 1);
    let mut pos: i64 = 0;
    let mut expands_seen = 0;
    for i in 0..(n - 1) {
        pos += stretch_data[i].final_size;
        if stretch_data[i].expand_flag {
            expands_seen += 1;
        }
        let dragger = if expands_seen == 0 {
            0
        } else if expands_seen >= expand_count {
            size_int - sep.round() as i64
        } else {
            pos
        };
        default_positions.push(dragger);
        pos += sep.round() as i64;
    }

    // ---- Step 2: 应用 split_offset + clamp（_update_dragger_positions）----
    // 简化：只支持单 dragger（split_offset 是单个值），对齐 Godot 单分割线场景。
    // dragger_positions[i] = CLAMP(default[i] + split_offset, valid_range.x, valid_range.y)
    let split_off = split_offset.round() as i64;
    let mut dragger_positions = Vec::with_capacity(n - 1);
    for i in 0..(n - 1) {
        let valid_range = split_valid_range(size_int, sep.round() as i64, min_sizes, i);
        let clamped = (default_positions[i] + split_off).clamp(valid_range.0, valid_range.1);
        dragger_positions.push(clamped);
    }

    // 防止相邻 dragger 重叠（对齐 Godot 第 864-875 行的 p_clamp_index == -1 分支）
    for i in 0..(dragger_positions.len().saturating_sub(1)) {
        let check_min = min_sizes[i + 1].round() as i64;
        let push_pos = dragger_positions[i] + sep.round() as i64 + check_min;
        if dragger_positions[i + 1] < push_pos {
            dragger_positions[i + 1] = push_pos;
        }
    }

    // ---- 把 dragger_positions 转成每个子节点的 final_size（_resort 第 963-982 行）----
    // start_pos[i] = i==0 ? 0 : dragger_positions[i-1] + sep
    // end_pos[i] = i >= n-1 ? size : dragger_positions[i]
    // size_i = end_pos - start_pos
    let mut final_sizes = Vec::with_capacity(n);
    for i in 0..n {
        let start = if i == 0 { 0 } else { dragger_positions[i - 1] + sep.round() as i64 };
        let end = if i >= n - 1 { size_int } else { dragger_positions[i] };
        final_sizes.push((end - start) as f32);
    }

    final_sizes
}

/// SplitContainer 的 valid_range（移植自 Godot `_get_valid_range`，简化版，忽略 max_size）。
///
/// 返回 (min_pos, max_pos)，dragger 位置 clamp 到此区间。
/// min_pos = sep*i + 左侧子节点 min_size 之和
/// max_pos = size - sep*(n-1-i) - 右侧子节点 min_size 之和
fn split_valid_range(size: i64, sep: i64, min_sizes: &[f32], dragger_index: usize) -> (i64, i64) {
    let n = min_sizes.len();
    let mut position_range = (0i64, size);
    position_range.0 += sep * dragger_index as i64;
    position_range.1 -= sep * ((n - 1 - dragger_index) as i64);

    for i in 0..n {
        let cms = min_sizes[i].round() as i64;
        if i <= dragger_index {
            position_range.0 += cms;
        } else {
            position_range.1 -= cms;
        }
    }
    position_range
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
        let child = LayoutNode::new("center", Preset::Center.to_anchors(), Offsets {
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
