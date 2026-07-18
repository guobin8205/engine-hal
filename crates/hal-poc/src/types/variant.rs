//! Godot `.tscn` 值字段的 Variant 类型。
//!
//! `.tscn` 的 `key = value` 行中，`value` 可以是多种类型：数字、字符串、
//! 几何类型（Vector2/Vector3/Color）、容器（数组/字典）、资源引用等。
//! 这里用 `Variant` 枚举统一表达。

use glam::{Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

/// `.tscn` 文件中可能出现的值类型。
///
/// 设计原则：
/// - 使用 `f32` 而非 `f64`：Godot 的 Vector/Color 在源码里都是 32 位浮点
/// - `Dict` 用 `Vec<(Variant, Variant)>` 而非 `HashMap`：保留声明顺序，
///   且 key 不一定可哈希（虽然实践中都是字符串）
/// - 资源引用用专门的 `ExtResource`/`SubResource`：保留 id 便于后续解析阶段解析
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Variant {
    /// `null` / `nil`
    Null,
    /// `true` / `false`
    Bool(bool),
    /// `42` / `-7`
    Int(i64),
    /// `0.5` / `1e3` / `1.0`
    Float(f32),
    /// `"hello"`（已反转义）
    String(String),
    /// `NodePath("UI/Panel")`
    NodePath(String),
    /// `&"name"`（Godot StringName）
    StringName(String),
    /// `Vector2(x, y)`
    Vector2(Vec2),
    /// `Vector2i(x, y)`
    Vector2i([i64; 2]),
    /// `Vector3(x, y, z)`
    Vector3(Vec3),
    /// `Vector3i(x, y, z)`
    Vector3i([i64; 3]),
    /// `Vector4(x, y, z, w)`（Godot 4 新增）
    Vector4(Vec4),
    /// `Vector4i(x, y, z, w)`
    Vector4i([i64; 4]),
    /// `Rect2(x, y, w, h)`
    Rect2([f32; 4]),
    /// `Rect2i(x, y, w, h)`
    Rect2i([i64; 4]),
    /// `Color(r, g, b, a)`，a 缺省时为 1.0；也对应 `#RRGGBBAA` 词法形式
    Color(Vec4),
    /// `AABB(position_xyz, size_xyz)` —— 6 real
    AABB([f32; 6]),
    /// `Transform2D` —— 6 real（两列 Vector2 + 平移）
    Transform2D([f32; 6]),
    /// `Plane(a, b, c, d)` —— 4 real
    Plane([f32; 4]),
    /// `Quaternion(x, y, z, w)` —— 4 real
    Quaternion([f32; 4]),
    /// `Basis` —— 9 real（3x3 矩阵）
    Basis([f32; 9]),
    /// `Transform3D` —— 12 real（Basis + 平移）
    Transform3D([f32; 12]),
    /// `Projection` —— 16 real（4x4 矩阵）
    Projection([f32; 16]),
    /// `RID()` / `RID(id)`
    RID(u64),
    /// `[v1, v2, v3]`（元素可为任意 Variant）
    Array(Vec<Variant>),
    /// 类型化数组 `Array[Type]([...])`：元素 + 类型标注（Godot 4 新增）
    TypedArray {
        elem_type: String,
        items: Vec<Variant>,
    },
    /// 32 位浮点打包数组：`PackedFloat32Array(0, 0.5, 1)`
    PackedFloat32Array(Vec<f32>),
    /// 64 位浮点打包数组：`PackedFloat64Array(...)`
    PackedFloat64Array(Vec<f64>),
    /// 32 位整数打包数组：`PackedInt32Array(0, 1, 2)`
    PackedInt32Array(Vec<i32>),
    /// 64 位整数打包数组：`PackedInt64Array(...)`
    PackedInt64Array(Vec<i64>),
    /// 2D 向量打包数组：`PackedVector2Array(...)`
    PackedVector2Array(Vec<Vec2>),
    /// 3D 向量打包数组：`PackedVector3Array(...)`
    PackedVector3Array(Vec<Vec3>),
    /// 4D 向量打包数组：`PackedVector4Array(...)`（format=4 新增）
    PackedVector4Array(Vec<Vec4>),
    /// 颜色打包数组：`PackedColorArray(...)`
    PackedColorArray(Vec<Vec4>),
    /// 字节数组：`PackedByteArray(...)`
    PackedByteArray(Vec<u8>),
    /// 字符串数组：`PackedStringArray(...)`
    PackedStringArray(Vec<String>),
    /// `{ "key": value, ... }`（保留声明顺序；key 可任意 Variant）
    Dict(Vec<(Variant, Variant)>),
    /// 类型化字典 `Dictionary[K, V]({...})`
    TypedDict {
        key_type: String,
        value_type: String,
        entries: Vec<(Variant, Variant)>,
    },
    /// `ExtResource("1_abc")` —— 文件内本地 id
    ExtResource(String),
    /// `SubResource("Name_xxx")` —— 文件内本地 id
    SubResource(String),
    /// 未知类型构造器：`Foo(1, 2, 3)` —— 保留类型名 + 参数（容错降级）
    UnknownConstructor {
        type_name: String,
        args: Vec<Variant>,
    },
}

impl Variant {
    /// 方便构造：标量浮点（避免到处写 `Variant::Float(x)`）。
    pub fn float(v: f32) -> Self {
        Variant::Float(v)
    }

    /// 方便构造：Vector2。
    pub fn vec2(x: f32, y: f32) -> Self {
        Variant::Vector2(Vec2::new(x, y))
    }

    /// 方便构造：Color（rgba 顺序，a 缺省 1.0）。
    pub fn color(r: f32, g: f32, b: f32, a: f32) -> Self {
        Variant::Color(Vec4::new(r, g, b, a))
    }

    /// 如果是 String/NodePath/StringName，返回内部字符串切片。
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Variant::String(s) | Variant::NodePath(s) | Variant::StringName(s) => Some(s),
            _ => None,
        }
    }

    /// 如果是 Float/Int，返回 f32。
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Variant::Float(f) => Some(*f),
            Variant::Int(i) => Some(*i as f32),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_equality_and_helpers() {
        assert_eq!(Variant::float(1.0), Variant::Float(1.0));
        assert_eq!(Variant::vec2(1.0, 2.0), Variant::Vector2(Vec2::new(1.0, 2.0)));
        assert_eq!(Variant::color(0.5, 0.5, 0.5, 1.0).as_f32(), None);

        assert_eq!(Variant::String("hi".into()).as_str(), Some("hi"));
        assert_eq!(Variant::NodePath(".".into()).as_str(), Some("."));
        assert_eq!(Variant::Float(3.14).as_f32(), Some(3.14));
        assert_eq!(Variant::Int(7).as_f32(), Some(7.0));
        assert_eq!(Variant::Bool(true).as_f32(), None);
    }

    #[test]
    fn variant_clone_is_deep() {
        let v = Variant::Array(vec![Variant::Int(1), Variant::String("x".into())]);
        let cloned = v.clone();
        assert_eq!(v, cloned);
    }
}
