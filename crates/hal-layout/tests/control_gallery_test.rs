//! 用 control_gallery.tscn（Godot 官方 demo，634 行，275 节点）压测 hal-layout。
//!
//! 这个测试验证 hal-layout 能处理真实的大型 Godot UI 场景。

use hal_layout::converter::build_layout_tree;
use hal_layout::layout_tree::Size;
use hal_poc::parse_scene;

fn load_control_gallery() -> hal_poc::SceneData {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../hal-poc/tests/fixtures/real/control_gallery.tscn");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 {}: {}", path.display(), e));
    parse_scene(&content).expect("解析 control_gallery 失败")
}

#[test]
fn control_gallery_builds_layout_tree() {
    let scene = load_control_gallery();
    let tree = build_layout_tree(&scene, Size::new(960.0, 640.0));

    assert!(tree.is_some(), "应该能构建布局树");
    let root = tree.unwrap();
    assert_eq!(root.computed.size, Size::new(960.0, 640.0), "Root 应填满窗口");
}

#[test]
fn control_gallery_flat_count() {
    let scene = load_control_gallery();
    let tree = build_layout_tree(&scene, Size::new(960.0, 640.0)).unwrap();
    let flat = tree.flatten();

    // control_gallery 有 ~120 个节点，但很多是嵌套的
    println!("control_gallery flat count = {}", flat.len());
    assert!(
        flat.len() > 50,
        "control_gallery 应该有 50+ 个节点，实际 {}",
        flat.len()
    );
}

#[test]
fn control_gallery_containers_detected() {
    let scene = load_control_gallery();
    let tree = build_layout_tree(&scene, Size::new(960.0, 640.0)).unwrap();

    // 统计容器类型
    let mut hbox_count = 0;
    let mut vbox_count = 0;
    let mut margin_count = 0;

    fn count_containers(node: &hal_layout::layout_tree::LayoutNode,
                        hbox: &mut usize, vbox: &mut usize, margin: &mut usize) {
        if let Some(c) = node.container {
            use hal_layout::layout_tree::ContainerType;
            match c {
                ContainerType::HBox { .. } => *hbox += 1,
                ContainerType::VBox { .. } => *vbox += 1,
                ContainerType::Margin { .. } => *margin += 1,
                ContainerType::HSplit { .. } => *hbox += 1,
                ContainerType::VSplit { .. } => *vbox += 1,
                ContainerType::Center => {}
                ContainerType::Tab { .. } => *margin += 1,
            }
        }
        for child in &node.children {
            count_containers(child, hbox, vbox, margin);
        }
    }

    count_containers(&tree, &mut hbox_count, &mut vbox_count, &mut margin_count);

    println!("HBox={} VBox={} Margin={}", hbox_count, vbox_count, margin_count);
    assert!(vbox_count > 0, "应该有 VBoxContainer");
    assert!(hbox_count > 0, "应该有 HBoxContainer");
}

#[test]
fn control_gallery_no_panic_or_nan() {
    // 确保布局过程不会 panic 或产生 NaN
    let scene = load_control_gallery();
    let tree = build_layout_tree(&scene, Size::new(960.0, 640.0)).unwrap();
    let flat = tree.flatten();

    let mut nan_count = 0;
    let mut zero_size_count = 0;
    let mut negative_size_count = 0;

    for node in &flat {
        if node.position.0.is_nan() || node.position.1.is_nan()
            || node.size.width.is_nan() || node.size.height.is_nan()
        {
            nan_count += 1;
            println!("⚠️ NaN: {}", node.name);
        }
        if node.size.width == 0.0 && node.size.height == 0.0 {
            zero_size_count += 1;
        }
        if node.size.width < 0.0 || node.size.height < 0.0 {
            negative_size_count += 1;
            println!("⚠️ 负尺寸: {} ({},{})", node.name, node.size.width, node.size.height);
        }
    }

    println!("NaN={} zero_size={} negative_size={}", nan_count, zero_size_count, negative_size_count);

    assert_eq!(nan_count, 0, "不应该有 NaN");
    assert_eq!(negative_size_count, 0, "不应该有负尺寸");
    // zero_size 是允许的（BottomWide 那种）
}

#[test]
fn control_gallery_dump_sample() {
    // 打印前 20 个节点的布局结果（人工检查用）
    let scene = load_control_gallery();
    let tree = build_layout_tree(&scene, Size::new(960.0, 640.0)).unwrap();
    let flat = tree.flatten();

    println!("=== control_gallery 布局结果（前 20 个）===");
    println!("{:<35} {:>8} {:>8} {:>8} {:>8}",
        "节点名", "x", "y", "width", "height");
    for node in flat.iter().take(20) {
        println!("{:<35} {:>8.0} {:>8.0} {:>8.0} {:>8.0}",
            node.name, node.position.0, node.position.1, node.size.width, node.size.height);
    }
}
