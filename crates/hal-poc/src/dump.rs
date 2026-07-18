//! 把 `SceneData` 转成人类可读的树状文本，方便调试和 POC 演示。

use std::fmt::Write;

use crate::types::{SceneData, Variant};

/// 把场景数据格式化为可读文本。
pub fn dump_scene(scene: &SceneData) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "=== Scene ===");
    let _ = writeln!(
        out,
        "header: format={} uid={:?} load_steps={:?}",
        scene.header.format, scene.header.uid, scene.header.load_steps
    );

    if !scene.ext_resources.is_empty() {
        let _ = writeln!(out, "\n--- ExtResources ({}) ---", scene.ext_resources.len());
        for er in &scene.ext_resources {
            let _ = writeln!(
                out,
                "  [{}] type={} path={} uid={:?}",
                er.id, er.r#type, er.path, er.uid
            );
        }
    }

    if !scene.sub_resources.is_empty() {
        let _ = writeln!(out, "\n--- SubResources ({}) ---", scene.sub_resources.len());
        for sr in &scene.sub_resources {
            let _ = writeln!(out, "  [{}] type={} ({} props)", sr.id, sr.r#type, sr.props.len());
            for (k, v) in &sr.props {
                let _ = writeln!(out, "    {} = {}", k, format_variant(v));
            }
        }
    }

    if !scene.nodes.is_empty() {
        let _ = writeln!(out, "\n--- Nodes ({}) ---", scene.nodes.len());
        for node in &scene.nodes {
            let type_str = node.r#type.as_deref().unwrap_or("(instance)");
            let parent_str = node.parent.as_deref().unwrap_or("(root)");
            let _ = writeln!(
                out,
                "  {} [type={}, parent={}] ({} props)",
                node.name,
                type_str,
                parent_str,
                node.props.len()
            );
            for (k, v) in &node.props {
                let _ = writeln!(out, "    {} = {}", k, format_variant(v));
            }
        }
    }

    if !scene.connections.is_empty() {
        let _ = writeln!(out, "\n--- Connections ({}) ---", scene.connections.len());
        for c in &scene.connections {
            let _ = writeln!(
                out,
                "  {}.{} -> {}.{}() flags={:?}",
                c.from, c.signal, c.to, c.method, c.flags
            );
        }
    }

    out
}

/// 把 Variant 格式化为单行可读字符串。
pub fn format_variant(v: &Variant) -> String {
    match v {
        Variant::Null => "null".into(),
        Variant::Bool(b) => b.to_string(),
        Variant::Int(i) => i.to_string(),
        Variant::Float(f) => format!("{}", f),
        Variant::String(s) => format!("{:?}", s),
        Variant::NodePath(p) => format!("NodePath({:?})", p),
        Variant::StringName(s) => format!("&{:?}", s),
        Variant::Vector2(v) => format!("Vector2({}, {})", v.x, v.y),
        Variant::Vector2i([x, y]) => format!("Vector2i({}, {})", x, y),
        Variant::Vector3(v) => format!("Vector3({}, {}, {})", v.x, v.y, v.z),
        Variant::Vector3i([x, y, z]) => format!("Vector3i({}, {}, {})", x, y, z),
        Variant::Vector4(v) => format!("Vector4({}, {}, {}, {})", v.x, v.y, v.z, v.w),
        Variant::Vector4i([x, y, z, w]) => format!("Vector4i({}, {}, {}, {})", x, y, z, w),
        Variant::Rect2([x, y, w, h]) => format!("Rect2({}, {}, {}, {})", x, y, w, h),
        Variant::Rect2i([x, y, w, h]) => format!("Rect2i({}, {}, {}, {})", x, y, w, h),
        Variant::Color(c) => format!("Color({}, {}, {}, {})", c.x, c.y, c.z, c.w),
        Variant::AABB(arr) => format!("AABB({})", arr.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ")),
        Variant::Transform2D(arr) => format!("Transform2D({})", arr.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ")),
        Variant::Plane(arr) => format!("Plane({})", arr.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ")),
        Variant::Quaternion(arr) => format!("Quaternion({})", arr.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ")),
        Variant::Basis(arr) => format!("Basis({})", arr.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ")),
        Variant::Transform3D(arr) => format!("Transform3D({})", arr.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ")),
        Variant::Projection(arr) => format!("Projection({})", arr.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ")),
        Variant::RID(id) => format!("RID({})", id),
        Variant::Array(items) => {
            let inner: Vec<String> = items.iter().map(format_variant).collect();
            format!("[{}]", inner.join(", "))
        }
        Variant::TypedArray { elem_type, items } => {
            let inner: Vec<String> = items.iter().map(format_variant).collect();
            format!("Array[{}]([{}])", elem_type, inner.join(", "))
        }
        Variant::PackedFloat32Array(items) => {
            format!("PackedFloat32Array({})", items.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", "))
        }
        Variant::PackedFloat64Array(items) => {
            format!("PackedFloat64Array({})", items.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", "))
        }
        Variant::PackedInt32Array(items) => {
            format!("PackedInt32Array({})", items.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", "))
        }
        Variant::PackedInt64Array(items) => {
            format!("PackedInt64Array({})", items.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", "))
        }
        Variant::PackedVector2Array(items) => {
            format!("PackedVector2Array({})", items.iter().map(|v| format!("({}, {})", v.x, v.y)).collect::<Vec<_>>().join(", "))
        }
        Variant::PackedVector3Array(items) => {
            format!("PackedVector3Array({})", items.iter().map(|v| format!("({}, {}, {})", v.x, v.y, v.z)).collect::<Vec<_>>().join(", "))
        }
        Variant::PackedVector4Array(items) => {
            format!("PackedVector4Array({})", items.iter().map(|v| format!("({}, {}, {}, {})", v.x, v.y, v.z, v.w)).collect::<Vec<_>>().join(", "))
        }
        Variant::PackedColorArray(items) => {
            format!("PackedColorArray({})", items.iter().map(|c| format!("({},{},{},{})", c.x, c.y, c.z, c.w)).collect::<Vec<_>>().join(", "))
        }
        Variant::PackedByteArray(items) => {
            format!("PackedByteArray({})", items.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(", "))
        }
        Variant::PackedStringArray(items) => {
            format!("PackedStringArray({})", items.iter().map(|s| format!("{:?}", s)).collect::<Vec<_>>().join(", "))
        }
        Variant::Dict(pairs) => {
            let inner: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("{}: {}", format_variant(k), format_variant(v)))
                .collect();
            format!("{{ {} }}", inner.join(", "))
        }
        Variant::TypedDict { key_type, value_type, entries } => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}: {}", format_variant(k), format_variant(v)))
                .collect();
            format!("Dictionary[{}, {}]({{ {} }})", key_type, value_type, inner.join(", "))
        }
        Variant::ExtResource(id) => format!("ExtResource({:?})", id),
        Variant::SubResource(id) => format!("SubResource({:?})", id),
        Variant::UnknownConstructor { type_name, args } => {
            let inner: Vec<String> = args.iter().map(format_variant).collect();
            format!("{}({})", type_name, inner.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SceneHeader;

    #[test]
    fn dump_empty_scene() {
        let scene = SceneData {
            header: SceneHeader {
                format: 3,
                uid: Some("uid://test".into()),
                load_steps: None,
            },
            ext_resources: vec![],
            sub_resources: vec![],
            nodes: vec![],
            connections: vec![],
            editable_instances: vec![],
        };
        let dump = dump_scene(&scene);
        assert!(dump.contains("format=3"));
        assert!(dump.contains("uid://test"));
    }

    #[test]
    fn format_variant_all_kinds() {
        assert_eq!(format_variant(&Variant::Null), "null");
        assert_eq!(format_variant(&Variant::Int(42)), "42");
        assert_eq!(format_variant(&Variant::Bool(true)), "true");
        assert_eq!(format_variant(&Variant::String("hi".into())), "\"hi\"");
        assert_eq!(
            format_variant(&Variant::ExtResource("1_abc".into())),
            "ExtResource(\"1_abc\")"
        );
    }
}
