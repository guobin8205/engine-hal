//! `.tscn` 场景的结构化表示。
//!
//! 一个 `.tscn` 文件由 5 个段组成：文件头、外部资源、内部资源、节点、信号连接。
//! 这里为每个段定义强类型结构，便于下游（POC-B 的 Cocos 重建器）消费。

use serde::{Deserialize, Serialize};

use super::Variant;

/// 完整场景数据，`.tscn` 文件解析后的根结构。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneData {
    /// 文件头：`[gd_scene format=3 uid="uid://..." load_steps=N]`
    pub header: SceneHeader,
    /// 所有 `[ext_resource ...]`，按文件出现顺序
    pub ext_resources: Vec<ExtResource>,
    /// 所有 `[sub_resource ...]`，按文件出现顺序
    pub sub_resources: Vec<SubResource>,
    /// 所有 `[node ...]`，按文件出现顺序（**未**构建父子树，保留 parent 字段）
    pub nodes: Vec<SceneNode>,
    /// 所有 `[connection ...]`
    pub connections: Vec<SceneConnection>,
    /// 所有 `[editable path="..."]`（标记哪些 instance 的子节点可被本场景编辑）
    pub editable_instances: Vec<String>,
}

/// `[gd_scene format=3 uid="uid://xxx" load_steps=N]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneHeader {
    /// `format=3`（Godot 4.x）或 `format=2`（Godot 3.x）
    pub format: u32,
    /// `uid="uid://..."`
    pub uid: Option<String>,
    /// `load_steps=N`（仅用于加载进度条，可选）
    pub load_steps: Option<u32>,
}

/// `[ext_resource type="Texture2D" path="res://x.png" id="1_abc"]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtResource {
    /// Godot 类型名，如 `Texture2D`、`Script`、`PackedScene`
    pub r#type: String,
    /// `res://...` 资源路径
    pub path: String,
    /// 文件内本地 id（Godot 4.x 是字符串如 `"1_abc"`）
    pub id: String,
    /// 可选的 `uid="uid://..."`
    pub uid: Option<String>,
}

/// `[sub_resource type="StyleBoxFlat" id="panel_xxx"]` 加上其下属属性。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubResource {
    /// Godot 资源类型名，如 `Animation`、`StyleBoxFlat`、`AnimationLibrary`
    pub r#type: String,
    /// 文件内本地 id
    pub id: String,
    /// 资源属性键值对，保留声明顺序
    pub props: Vec<(String, Variant)>,
}

/// `[node name="Hero" type="Sprite" parent="." instance=ExtResource("1_xxx")]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneNode {
    /// 节点名
    pub name: String,
    /// 节点类型（`type`）；若节点是 `instance` 的根，则可能缺失
    pub r#type: Option<String>,
    /// 父路径：`.` 表示场景根的直接子节点；`"Path/To"` 表示相对路径
    pub parent: Option<String>,
    /// `index=N`（可选）
    pub index: Option<i32>,
    /// `instance=ExtResource("id")`（若该节点是实例化的外部场景）
    pub instance: Option<String>,
    /// `instance_placeholder="res://..."`（与 instance 互斥）
    pub instance_placeholder: Option<String>,
    /// `owner="..."`（可选，实例化场景时用）
    pub owner: Option<String>,
    /// `unique_id=N`（Godot 4.4+，稳定场景节点 ID）
    pub unique_id: Option<i64>,
    /// `groups=["g1", "g2"]`（节点分组）
    pub groups: Vec<String>,
    /// `node_paths=[...]`（延迟应用的 NodePath 属性名列表）
    pub deferred_node_paths: Vec<String>,
    /// 节点自身属性键值对，保留声明顺序
    pub props: Vec<(String, Variant)>,
}

/// `[connection signal="pressed" from="Button" to="." method="_on_pressed"]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneConnection {
    /// 信号名
    pub signal: String,
    /// 源节点路径（相对场景根）
    pub from: String,
    /// 目标节点路径（相对场景根）
    pub to: String,
    /// 目标方法名
    pub method: String,
    /// `flags=N`（连接位标志：DEFERRED=1, PERSIST=2, ONE_SHOT=4）
    /// 注意：Godot 写入时若 flags==2（PERSIST）会省略该字段
    pub flags: Option<i32>,
    /// `binds=[...]`（绑定参数数组，元素是任意 Variant）
    pub binds: Vec<Variant>,
    /// `unbinds=N`（解绑最后 N 个参数，默认 0）
    pub unbinds: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scene_data() {
        let scene = SceneData {
            header: SceneHeader {
                format: 3,
                uid: None,
                load_steps: None,
            },
            ext_resources: vec![],
            sub_resources: vec![],
            nodes: vec![],
            connections: vec![],
            editable_instances: vec![],
        };
        assert_eq!(scene.nodes.len(), 0);
        assert_eq!(scene.header.format, 3);
    }

    #[test]
    fn node_with_props() {
        let node = SceneNode {
            name: "Hero".into(),
            r#type: Some("Sprite".into()),
            parent: Some(".".into()),
            index: None,
            instance: None,
            instance_placeholder: None,
            owner: None,
            unique_id: None,
            groups: vec![],
            deferred_node_paths: vec![],
            props: vec![
                ("position".into(), Variant::vec2(100.0, 200.0)),
                ("visible".into(), Variant::Bool(true)),
            ],
        };
        assert_eq!(node.name, "Hero");
        assert_eq!(node.props.len(), 2);
        assert_eq!(
            node.props[0],
            ("position".into(), Variant::vec2(100.0, 200.0))
        );
    }
}
