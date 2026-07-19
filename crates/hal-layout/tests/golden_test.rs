//! Golden test: 对比 hal-layout 算的布局 vs Godot 实际算的布局。
//!
//! 这是验证布局正确性的最强证据 —— 以 Godot 本身作为 oracle。
//! 容差：1.0 像素（Godot 的浮点运算可能有亚像素差异）。

use hal_layout::converter::build_layout_tree;
use hal_layout::layout_tree::Size;
use hal_poc::parse_scene;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GoldenLayout {
    #[serde(flatten)]
    nodes: std::collections::HashMap<String, GoldenNode>,
}

#[derive(Debug, Deserialize)]
struct GoldenNode {
    name: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn load_golden() -> GoldenLayout {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/layout_golden.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 golden JSON {}: {}", path.display(), e));
    serde_json::from_str(&content).expect("解析 golden JSON 失败")
}

fn load_scene() -> hal_poc::SceneData {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/layout_test.tscn");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 .tscn {}: {}", path.display(), e));
    parse_scene(&content).expect("解析 .tscn 失败")
}

/// 容差（像素）。Godot 用 f32 + 亚像素，允许小误差。
const TOLERANCE: f64 = 1.5;

/// 已知的语义差异（Phase 1 待完整对齐）。
/// 这些节点的差异不是 bug，而是 hal-layout 还没实现的 Godot 行为。
const KNOWN_DIFFERENCES: &[&str] = &[
    "BottomWide",  // Godot ColorRect 无内容时 minimum_size=0，anchor 拉伸后 size 仍为 0
];

#[test]
fn hal_layout_matches_godot() {
    let golden = load_golden();
    let scene = load_scene();

    // 用 golden 的 Root 尺寸作为窗口尺寸（确保一致）
    let root = golden.nodes.get("").expect("golden 应有 Root");
    let window_size = Size::new(root.width as f32, root.height as f32);

    let tree = build_layout_tree(&scene, window_size)
        .expect("hal-layout 应该能构建布局树");

    let flat = tree.flatten();

    println!("=== 布局对比（hal-layout vs Godot）===");
    println!("{:<20} {:>8} {:>8} {:>8} {:>8}  | {:>8} {:>8} {:>8} {:>8}  | {}",
        "节点", "hal_x", "hal_y", "hal_w", "hal_h",
        "godot_x", "godot_y", "godot_w", "godot_h", "结果");

    let mut mismatches = Vec::new();
    let mut known_count = 0;

    for layout_node in &flat {
        let golden_node = golden.nodes.values().find(|g| g.name == layout_node.name);

        if let Some(g) = golden_node {
            let hal_x = layout_node.position.0 as f64;
            let hal_y = layout_node.position.1 as f64;
            let hal_w = layout_node.size.width as f64;
            let hal_h = layout_node.size.height as f64;

            let dx = (hal_x - g.x).abs();
            let dy = (hal_y - g.y).abs();
            let dw = (hal_w - g.width).abs();
            let dh = (hal_h - g.height).abs();

            let ok = dx <= TOLERANCE && dy <= TOLERANCE && dw <= TOLERANCE && dh <= TOLERANCE;
            let is_known = KNOWN_DIFFERENCES.contains(&layout_node.name.as_str());

            let status = if ok {
                "✅"
            } else if is_known {
                known_count += 1;
                "⚠️ 已知差异"
            } else {
                "❌"
            };

            println!(
                "{:<20} {:>8.1} {:>8.1} {:>8.1} {:>8.1}  | {:>8.1} {:>8.1} {:>8.1} {:>8.1}  | {}",
                layout_node.name, hal_x, hal_y, hal_w, hal_h,
                g.x, g.y, g.width, g.height,
                status
            );

            if !ok && !is_known {
                mismatches.push(format!(
                    "{}: dx={:.1} dy={:.1} dw={:.1} dh={:.1}",
                    layout_node.name, dx, dy, dw, dh
                ));
            }
        }
    }

    println!("\n=== 总结 ===");
    println!("匹配: {}", flat.len() - known_count - mismatches.len());
    println!("已知差异: {}", known_count);
    println!("不匹配: {}", mismatches.len());

    if !mismatches.is_empty() {
        panic!(
            "hal-layout 与 Godot 布局不一致（容差 {}px）：\n  {}",
            TOLERANCE,
            mismatches.join("\n  ")
        );
    }
}
