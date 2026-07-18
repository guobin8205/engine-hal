# hal-poc

> Engine-HAL POC 的 Rust 部分：解析 Godot `.tscn` 场景文件。

## 当前状态：POC-A 完成 ✅（v2，Godot 源码对齐版）

**实现方式**：手写递归下降 + 词法器，**忠实移植 Godot `VariantParser`**
（`core/variant/variant_parser.cpp` + `scene/resources/resource_format_text.cpp`）。

> ⚠️ **方向演化**：初版（v1）用 pest PEG 文法，但因为 PEG 难以表达 Godot 词法层的
> 细节（StringName 是词法 token、`#hex` Color 是词法 token、未知转义=字面量等），
> 在用真实 Godot demo 项目验证时暴露出严重缺陷，遂重写为 Godot 源码移植版（v2）。
> 详见 [`../docs/godot-source-comparison.md`](../docs/godot-source-comparison.md)。

## 验证证据

### ✅ 用真实 Godot 文件验证通过

测试集：[`godot-demo-projects/gui/control_gallery/control_gallery.tscn`](https://github.com/godotengine/godot-demo-projects/blob/master/gui/control_gallery/control_gallery.tscn)（Godot 官方 demo，634 行真实文件）

完整覆盖以下 Godot 4 语法特性：

| 特性 | 验证状态 |
|---|---|
| `[gd_scene format=3 uid="..."]` 文件头 | ✅ |
| `[ext_resource type/path/id/uid]` | ✅ |
| `[sub_resource type/id]` + 多个属性 | ✅ |
| `[node name/type/parent/unique_id]`（含 Godot 4.4 unique_id） | ✅ |
| `[connection signal/from/to/method]` | ✅ |
| 标量（int/float/bool/null/nil/inf/-inf/nan） | ✅ |
| 字符串（含 `\b \t \n \f \r \uXXXX \UXXXXXX` 转义 + UTF-16 代理对） | ✅ |
| 未知转义 = 字面量（Godot 兼容行为） | ✅ |
| 多行字符串（含真实换行） | ✅ |
| Unicode / 中文 / emoji | ✅ |
| StringName `&"..."`（词法层 token） | ✅ |
| NodePath `"..."`（构造器形式） | ✅ |
| `#RRGGBBAA` Color（词法层 token） | ✅ |
| 几何类型（Vector2/3/4、Rect2、Color、Plane、Quaternion、AABB、Basis、Transform2D/3D、Projection） | ✅ |
| 整数版几何（Vector2i/3i/4i、Rect2i） | ✅ |
| 数组（含嵌套、空、尾逗号） | ✅ |
| 字典（含多行、嵌套、任意 key 类型） | ✅ |
| **类型化数组 `Array[Type]([...])`** | ✅ |
| **类型化字典 `Dictionary[K,V]({...})`** | ✅ |
| Packed 数组（Float32/64、Int32/64、Vector2/3/4、Color、Byte、String） | ✅ |
| 资源引用 ExtResource/SubResource | ✅ |
| 节点 tag 字段：groups、node_paths、instance_placeholder、unique_id | ✅ |
| 属性 key 含 `/`（如 `theme_override_styles/panel`） | ✅ |
| 未知构造器降级（`UnknownConstructor`） | ✅ |
| Godot 3.x 别名（Quat、Matrix3、Pool*Array 等） | ✅ |

### 测试统计

- **单元测试**：14 个（数据模型 + Stream + 词法器）
- **真实集成测试**：2 个（用 Godot 官方 control_gallery.tscn 验证）
- **总通过率**：100%（16/16）

## 用法

### 命令行工具

```bash
# dump 一个 .tscn 文件
cargo run --example dump_tscn -- path/to/scene.tscn

# JSON 输出
cargo run --example dump_tscn -- --json path/to/scene.tscn
```

### 库 API

```rust
use hal_poc::parse_scene;

let scene = parse_scene(&std::fs::read_to_string("scene.tscn")?)?;
println!("节点数: {}", scene.nodes.len());
for node in &scene.nodes {
    println!("  {} [type={:?}] unique_id={:?}",
        node.name, node.r#type, node.unique_id);
}
```

## 架构（与 Godot 源码对齐）

```
src/parser/
├── stream.rs         ← 字符流（对应 Godot VariantParser::Stream）
├── lexer.rs          ← 词法器（对应 Godot get_token，含 #Color、&"..."、数字状态机）
├── value.rs          ← Variant 值解析（对应 Godot parse_value + _parse_construct 等）
└── scene_parser.rs   ← 顶层场景解析（对应 Godot ResourceLoaderText + parse_tag_assign_eof）
```

**移植对应表**：

| hal-poc (Rust) | Godot (C++) | 文件位置 |
|---|---|---|
| `Stream` | `VariantParser::Stream` | `core/variant/variant_parser.cpp` |
| `get_token()` | `VariantParser::get_token()` | 同上，162-520 行 |
| `parse_value()` | `VariantParser::parse_value()` | 同上，677-1642 行 |
| `parse_construct_real/int/string()` | `_parse_construct<T>()` | 同上，552-599 行 |
| `parse_dictionary_contents()` | `_parse_dictionary()` | 同上，1684-1748 行 |
| `parse_array_contents()` | `_parse_array()` | 同上，1644-1682 行 |
| `parse_tag()` | `_parse_tag()` | 同上，1750-1873 行 |
| `parse_tag_or_assign()` | `parse_tag_assign_eof()` | 同上，1891-1961 行 |
| `SceneLoader::handle_tag()` | `ResourceLoaderText::_parse_node_tag()` | `scene/resources/resource_format_text.cpp` |

## 关键设计决策（与 Godot 对齐）

1. **手写递归下降**，不用 pest PEG
   - Godot 词法层有"动态语义"（`#` 是 Color 而非注释、`&"..."` 是 StringName、未知转义=字面量），PEG 难以表达
   - 手写让每个细节都对齐 Godot 源码

2. **Typed collection 作为独立 Variant**
   - `Array[Type]([...])` 和 `Dictionary[K,V]({...})` 在 Variant 枚举里有专门的 `TypedArray`/`TypedDict` 变体
   - 保留类型信息，不降级为普通 Array/Dict

3. **未知构造器降级**
   - Godot 遇到未知构造器会报错中止；hal-poc 降级为 `UnknownConstructor { type_name, args }`
   - 适用于 POC 阶段的容错需求，将来可改为严格模式

4. **PackedByteArray 的 base64 形式**
   - POC 阶段不实际解码，保留为空数组（标注 TODO）

## 仍有的差距（Godot 完整支持的少量特性）

- ❌ 二进制 `.res` 文件（只支持文本 `.tscn`/`.tres`）
- ❌ `instance=ExtResource(...)` 节点的属性合并语义（只保留 instance 引用）
- ❌ MissingNode 兼容（未知节点类型 fallback）
- ❌ Node 树的显式构建（只保留 parent 字段）
- ❌ base64 PackedByteArray 的实际解码

这些是 Phase 1+ 的工作，POC 阶段的核心目标（证明 Rust 能解析真实 Godot .tscn）已达成。

## 经验教训（POC-A 全程记录）

1. **手写 fixtures 是危险的** —— 初版用自己写的 fixtures，全过但实际无法解析真实 Godot 文件
2. **官方源码是最好的文档** —— 比 Godot 文档网站详细 10 倍，且永远准确
3. **PEG 不适合 Godot 语法** —— 词法层的动态语义让 pest 文法复杂且易错
4. **真实数据是最便宜的验证** —— 用 Godot 编辑器导出的真实文件比 100 个手写测试都有用
5. **移植优于重写** —— 对齐 Godot 源码让所有边界情况自动正确

## 下一步：POC-B

POC-A 验证了"Rust 能解析真实 .tscn"。POC-B 验证"Rust 能否通过 cxx 调用 Cocos2d-x C++ API 显示场景"。

- 引擎：`E:/repos/cocos/cocos2d-x-3.15.1`
- 最大风险：cxx 与 Cocos C++ 模板的兼容性、MSVC 工具链对齐
- 前置条件：POC-A ✅ 已通过
