//! 真实 .tscn 文件集成测试：用 Godot 官方 demo 项目的输出验证解析器对齐 Godot。

use hal_poc::{parse_scene, Variant};

fn read_fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 fixture {}: {}", path.display(), e))
}

#[test]
fn real_control_gallery_parses() {
    let content = read_fixture("real/control_gallery.tscn");
    let scene = parse_scene(&content).expect("真实 .tscn 解析失败");

    // 文件头
    assert_eq!(scene.header.format, 3);
    assert_eq!(scene.header.uid.as_deref(), Some("uid://dqguuy0aao4cr"));

    // 2 个 ext_resource（icon.webp + tree.gd）
    assert_eq!(scene.ext_resources.len(), 2);
    assert_eq!(scene.ext_resources[0].r#type, "Texture2D");
    assert_eq!(scene.ext_resources[0].path, "res://icon.webp");
    assert_eq!(scene.ext_resources[0].id, "1_8tycj");

    // 多个 sub_resource（StyleBoxFlat / CodeHighlighter / ButtonGroup / FoldableGroup）
    assert!(scene.sub_resources.len() >= 4);
    let style_box = scene
        .sub_resources
        .iter()
        .find(|s| s.r#type == "StyleBoxFlat")
        .expect("应有 StyleBoxFlat");
    assert_eq!(style_box.id, "StyleBoxFlat_bl4wp");

    // 验证 unique_id 字段（Godot 4.4+）
    let root = &scene.nodes[0];
    assert_eq!(root.name, "ControlGallery");
    assert_eq!(root.r#type.as_deref(), Some("Control"));
    assert_eq!(root.unique_id, Some(78764481));

    // 验证多行字典（CodeHighlighter 的 keyword_colors）
    let highlighter = scene
        .sub_resources
        .iter()
        .find(|s| s.r#type == "CodeHighlighter")
        .expect("应有 CodeHighlighter");
    let keyword_colors = highlighter
        .props
        .iter()
        .find(|(k, _)| k == "keyword_colors")
        .map(|(_, v)| v)
        .expect("应有 keyword_colors");
    match keyword_colors {
        Variant::Dict(entries) => {
            assert_eq!(entries.len(), 2, "keyword_colors 应有 2 个 entry");
            // 第一个 key 是 "func"
            match &entries[0].0 {
                Variant::String(s) => assert_eq!(s, "func"),
                other => panic!("dict key 应是 String, 实际 {:?}", other),
            }
        }
        other => panic!("keyword_colors 应是 Dict, 实际 {:?}", other),
    }

    // 验证 typed array：line_length_guidelines = Array[int]([24])
    let code_edit_node = scene
        .nodes
        .iter()
        .find(|n| n.name == "CodeEdit")
        .expect("应有 CodeEdit 节点");
    let typed_arr = code_edit_node
        .props
        .iter()
        .find(|(k, _)| k == "line_length_guidelines")
        .map(|(_, v)| v)
        .expect("CodeEdit 应有 line_length_guidelines");
    match typed_arr {
        Variant::TypedArray { elem_type, items } => {
            assert_eq!(elem_type, "int");
            assert_eq!(items.len(), 1);
            assert_eq!(items[0], Variant::Int(24));
        }
        other => panic!("应是 TypedArray, 实际 {:?}", other),
    }

    // 验证带 / 的属性 key
    let label_with_theme_override = scene.nodes.iter().find(|n| {
        n.props
            .iter()
            .any(|(k, _)| k.contains("theme_override_colors/font_color"))
    });
    assert!(
        label_with_theme_override.is_some(),
        "应能解析带 / 的属性 key"
    );

    // 验证多行字符串（含 \n）
    let label_with_multiline = scene
        .nodes
        .iter()
        .flat_map(|n| n.props.iter())
        .find(|(k, v)| {
            k == "text" && matches!(v, Variant::String(s) if s.contains('\n'))
        });
    assert!(
        label_with_multiline.is_some(),
        "应能解析含 \\n 的多行字符串"
    );
}

#[test]
fn real_control_gallery_node_count() {
    let content = read_fixture("real/control_gallery.tscn");
    let scene = parse_scene(&content).expect("解析失败");
    // 这个 demo 有大量节点
    println!(
        "control_gallery: {} nodes, {} sub_resources, {} ext_resources",
        scene.nodes.len(),
        scene.sub_resources.len(),
        scene.ext_resources.len()
    );
    assert!(
        scene.nodes.len() > 50,
        "control_gallery 应有 50+ 个节点"
    );
}
