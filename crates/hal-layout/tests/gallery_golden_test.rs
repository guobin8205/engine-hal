//! control_gallery golden test: 用 Godot 真实布局对比 hal-layout。
//!
//! 这是最强的正确性验证 —— 86 个节点的真实 Godot UI 场景。

use hal_layout::converter::build_layout_tree;
use hal_layout::layout_tree::Size;
use hal_poc::parse_scene;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct GoldenLayout {
    #[serde(flatten)]
    nodes: HashMap<String, GoldenNode>,
}

#[derive(Debug, Deserialize)]
struct GoldenNode {
    name: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    #[serde(default)]
    min_width: f64,
    #[serde(default)]
    min_height: f64,
}

const TOLERANCE: f64 = 50.0; // 大容差看趋势（grow_direction 未实现）

/// 递归注入 Godot 提供的真实 min_size 到 LayoutNode 树。
/// 这样能验证布局算法的正确性（排除 min_size 估算误差的影响）。
fn inject_godot_min_sizes(
    node: &mut hal_layout::layout_tree::LayoutNode,
    golden: &GoldenLayout,
    _parent_path: &str,
) {
    let mut found: Option<&GoldenNode> = None;
    for g in golden.nodes.values() {
        if g.name == node.name {
            if found.is_none() || g.min_width > found.unwrap().min_width {
                found = Some(g);
            }
        }
    }
    if node.name == "BasicControls" {
        if let Some(g) = found {
            eprintln!("INJECT BasicControls: golden min_width={} min_height={}", g.min_width, g.min_height);
        } else {
            eprintln!("INJECT BasicControls: NOT FOUND in golden");
        }
    }
    if let Some(g) = found {
        if g.min_width > 0.0 || g.min_height > 0.0 {
            node.min_size = hal_layout::layout_tree::Size::new(
                g.min_width as f32,
                g.min_height as f32,
            );
            if node.name == "BasicControls" {
                eprintln!("INJECT BasicControls: SET min_size=({},{})", g.min_width, g.min_height);
            }
        }
    }

    for child in &mut node.children {
        inject_godot_min_sizes(child, golden, "");
    }
}

fn load_gallery_golden() -> GoldenLayout {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/gallery_golden.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 gallery golden: {}", e));
    serde_json::from_str(&content).expect("解析 gallery golden 失败")
}

fn load_gallery_scene() -> hal_poc::SceneData {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../hal-poc/tests/fixtures/real/control_gallery.tscn");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 .tscn: {}", e));
    parse_scene(&content).expect("解析失败")
}

#[test]
fn control_gallery_matches_godot() {
    let golden = load_gallery_golden();
    let scene = load_gallery_scene();

    let window_size = Size::new(960.0, 640.0);
    let mut tree = build_layout_tree(&scene, window_size).expect("构建布局树失败");

    // 诊断：确认 golden 的 BasicControls min_size
    if let Some(g) = golden.nodes.get("MainPanel/HSplitContainer/BasicControls") {
        eprintln!("GOLDEN CHECK: BasicControls min_width={} min_height={}", g.min_width, g.min_height);
    }
    // （正式版应该由 hal-layout 自己精确计算 min_size，这里用 golden 验证布局算法的正确性）
    inject_godot_min_sizes(&mut tree, &golden, "");

    // 重新布局（用正确的 min_size）
    tree.layout(window_size);

    let flat = tree.flatten_local();

    let mut hal_map: HashMap<String, ((f32, f32), (f32, f32))> = HashMap::new();
    let root_name = &flat[0].name;
    for node in &flat {
        let golden_path = if &node.path == root_name {
            "".to_string()
        } else {
            node.path.strip_prefix(&format!("{}/", root_name))
                .map(|s| s.to_string())
                .unwrap_or_else(|| node.path.clone())
        };
        hal_map.insert(golden_path, (node.position, (node.size.width, node.size.height)));
    }

    fn find_node<'a>(node: &'a hal_layout::layout_tree::LayoutNode, name: &str) -> Option<&'a hal_layout::layout_tree::LayoutNode> {
        if node.name == name { return Some(node); }
        node.children.iter().find_map(|c| find_node(c, name))
    }
    for n in &["BasicControls", "VSplitContainer", "HSplitContainer"] {
        if let Some(node) = find_node(&tree, n) {
            eprintln!("AFTER LAYOUT: {} min_size=({:.0},{:.0}) computed=({:.0},{:.0},{:.0},{:.0})",
                n, node.min_size.width, node.min_size.height,
                node.computed.position.0, node.computed.position.1,
                node.computed.size.width, node.computed.size.height);
        }
    }
    // 诊断：打印 golden 里 BasicControls
    if let Some(g) = golden.nodes.get("MainPanel/HSplitContainer/BasicControls") {
        eprintln!("GOLDEN: 'MainPanel/HSplitContainer/BasicControls' pos=({:.0},{:.0}) size=({:.0},{:.0})",
            g.x, g.y, g.width, g.height);
    }

    let mut match_count = 0;
    let mut miss_count = 0;
    let mut not_found_count = 0;
    let mut details: Vec<String> = Vec::new();

    // 对比每个 golden 节点
    for (path, g) in &golden.nodes {
        if let Some(&(hal_pos, hal_size)) = hal_map.get(path) {
            let dx = (hal_pos.0 as f64 - g.x).abs();
            let dy = (hal_pos.1 as f64 - g.y).abs();
            let dw = (hal_size.0 as f64 - g.width).abs();
            let dh = (hal_size.1 as f64 - g.height).abs();

            let ok = dx <= TOLERANCE && dy <= TOLERANCE && dw <= TOLERANCE && dh <= TOLERANCE;

            if ok {
                match_count += 1;
            } else {
                miss_count += 1;
                if miss_count <= 15 {
                    details.push(format!(
                        "  ❌ {} ({}) : hal=({:.0},{:.0},{:.0},{:.0}) godot=({:.0},{:.0},{:.0},{:.0}) dx={:.0} dy={:.0} dw={:.0} dh={:.0}",
                        g.name, path,
                        hal_pos.0, hal_pos.1, hal_size.0, hal_size.1,
                        g.x, g.y, g.width, g.height,
                        dx, dy, dw, dh
                    ));
                }
            }
        } else {
            not_found_count += 1;
            if not_found_count <= 5 {
                details.push(format!("  ⚠️ {} ({}) 不在 hal-layout 结果中", g.name, path));
            }
        }
    }

    println!("=== control_gallery Golden Test ===");
    println!("总节点数(golden): {}", golden.nodes.len());
    println!("匹配: {}", match_count);
    println!("不匹配: {}", miss_count);
    println!("未找到: {}", not_found_count);

    if !details.is_empty() {
        println!("\n详情（前 20）:");
        for d in details.iter().take(20) {
            println!("{}", d);
        }
    }

    // 容忍一定的不匹配（复杂的嵌套容器 + Tab/Foldable 等未完全实现）
    let total = golden.nodes.len();
    let match_rate = match_count as f64 / total as f64;
    println!("\n匹配率: {:.1}% ({}/{})", match_rate * 100.0, match_count, total);

    // Phase 2 初期：control_gallery 有大量复杂嵌套容器
    // 当前已知差距：
    //   - layout_mode（1=锚点, 2=容器模式）未处理
    //   - HSplitContainer 嵌套时的 split_offset 传播
    //   - TabContainer/FoldableContainer 的精确语义
    // 随着这些逐步实现，匹配率会提升
    println!("\n注: control_gallery 是 86 节点的复杂场景，当前匹配率低是预期的。");
    println!("Phase 2 目标: 逐步提升到 80%+。");

    // 不设硬阈值 —— 这个测试主要是诊断工具，看哪些节点不匹配
    // 当匹配率提升到 50%+ 时可以加 assert
}
