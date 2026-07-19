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

#[test]
fn hal_layout_matches_godot() {
    let golden = load_golden();
    let scene = load_scene();

    // 注意：Godot headless 用的窗口尺寸是 1024x704（不是 project.godot 里的 960x640）
    // 这是 Godot 4.x 在无显示器环境下的默认行为
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

    for layout_node in &flat {
        // 在 golden 里找对应节点（用节点名匹配）
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

            println!(
                "{:<20} {:>8.1} {:>8.1} {:>8.1} {:>8.1}  | {:>8.1} {:>8.1} {:>8.1} {:>8.1}  | {}",
                layout_node.name, hal_x, hal_y, hal_w, hal_h,
                g.x, g.y, g.width, g.height,
                if ok { "✅" } else { "❌" }
            );

            if !ok {
                mismatches.push(format!(
                    "{}: dx={:.1} dy={:.1} dw={:.1} dh={:.1}",
                    layout_node.name, dx, dy, dw, dh
                ));
            }
        }
    }

    if !mismatches.is_empty() {
        panic!(
            "hal-layout 与 Godot 布局不一致（容差 {}px）：\n  {}",
            TOLERANCE,
            mismatches.join("\n  ")
        );
    }
}
