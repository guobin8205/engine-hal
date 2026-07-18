# Godot 源码对比分析：hal-poc 解析器与官方实现

> **报告日期**: 2026-07-19
> **对比对象**:
> - hal-poc 的 pest 文法（`crates/hal-poc/src/parser/tscn.pest`）
> - Godot 官方 `VariantParser`（`E:/repos/godot/godot/core/variant/variant_parser.cpp`，2575 行）
> - Godot 官方 `ResourceLoaderText`（`E:/repos/godot/godot/scene/resources/resource_format_text.cpp`，2248 行）
> - Godot 官方 `PackedScene`（`E:/repos/godot/godot/scene/resources/packed_scene.cpp`，2631 行）

## 关键结论

**hal-poc 当前只覆盖了 Godot 真实语法的约 60%**。17 个测试全过是因为 fixtures 是"我以为"的语法，不是 Godot 实际输出的语法。用真实 Godot 编辑器导出的 .tscn 跑当前解析器，**很可能失败**。

## P0 盲点（阻塞解析真实 .tscn）

### P0-1: 类型化数组 / 类型化字典

Godot 4 序列化 typed collection 的形式是 **双层括号**：

```
# 不是这样
textures = [...] as Array[ExtResource]

# 而是这样（双层括号！）
textures = Array[ExtResource]([ExtResource("1_xxx"), ExtResource("2_yyy")])
settings = Dictionary[String, int]({"hp": 100, "mp": 50})
```

源码：`variant_parser.cpp:1188`（Dictionary）、`1328`（Array）。

**hal-poc 当前完全不支持**。Godot 4 的 .tres 文件里 typed array 极常见（任何 `@export var x: Array[Node]` 都会序列化成这种形式）。

### P0-2: `nil` 是 null 的别名

```
value = nil    # 等价于 null
```

源码：`variant_parser.cpp:700`。

**hal-poc 只支持 `null`**，遇到 `nil` 会报错。

### P0-3: 尾逗号

```
arr = [1, 2, 3,]      # 合法
dict = {"a": 1,}       # 合法
```

源码：`variant_parser.cpp:1672`（数组）、`1740`（字典）。

**hal-poc 当前不支持**。Godot Writer 实际会输出尾逗号（VCS 友好）。

### P0-4: 未识别转义 = 字面量

源码：`variant_parser.cpp:351-353`：

```cpp
default:
    // 未知转义当作字面量字符
    str.push_back(previous_char);  // '\'
    str.push_back(char);           // 'x'
```

**hal-poc 的 `unescape` 实现错了**：遇到未知转义会丢失反斜杠。比如 `"path\/file"` 在 Godot 里是 `path/file`（字面 `/`），hal-poc 会变成 `pathfile`。

正确实现：
```rust
// 已知转义
'n' => '\n', 't' => '\t', 'r' => '\r', 'b' => '\u{0008}', 'f' => '\u{000C}',
'\\' => '\\', '"' => '"', '\'', '\'',
'u' => { /* 4 位 hex */ },
'U' => { /* 6 位 hex */ },
// 未知：保留反斜杠 + 字面字符
_ => { out.push('\\'); out.push(c); }
```

## P1 - 字符串/数字语法对齐

### P1-1: Color 的 `#hex` 词法形式

```
font_color = #FFFFFFFF      # 8 位 RGBA hex
bg = #FF0000FF              # 红色不透明
```

源码：`variant_parser.cpp:242-262`（TK_COLOR token）。

**这是词法层的，不是构造器**。`#` 永远是 Color 前缀（不是注释）。hal-poc 完全没支持。

### P1-2: `inf` / `-inf` / `nan` 标识符

```
divisor = inf
offset = -inf
broken = nan
```

源码：`variant_parser.cpp:702-708`。

**这些是标识符，不是数字字面量**。hal-poc 没支持。

### P1-3: 字符串转义补全

| 转义 | 含义 | hal-poc 是否支持 |
|---|---|---|
| `\b` | 退格 0x08 | ❌ |
| `\f` | 换页 0x0C | ❌ |
| `\uXXXX` | 4 位 hex Unicode | ❌ |
| `\UXXXXXX` | 6 位 hex Unicode | ❌ |

源码：`variant_parser.cpp:289-354`。

### P1-4: 多行字符串允许裸换行

源码：`variant_parser.cpp:388-391`。Godot 字符串内部允许真实的换行字符（Writer 用 `c_escape_multiline()` 输出）。

**hal-poc 的 quoted_str 规则用 `(!("\"" | "\\") ~ ANY)` 包含 NEWLINE**，应该是对的，但需要测试验证。

## P2 - 类型补全

### 漏掉的几何类型（都是 `Type(n1, n2, ...)` 形式）

| 类型 | 参数数 | hal-poc 是否支持 |
|---|---|---|
| `Vector4` / `Vector4i` | 4 | ❌ |
| `Rect2i` | 4 | ❌ |
| `AABB`（别名 `Rect3`） | 6 | ❌ |
| `Plane` | 4 | ❌ |
| `Quaternion`（别名 `Quat`） | 4 | ❌ |
| `Basis`（别名 `Matrix3`） | 9 | ❌ |
| `Transform2D`（别名 `Matrix32`） | 6 | ❌ |
| `Transform3D`（别名 `Transform`） | 12 | ❌ |
| `Projection` | 16 | ❌ |

### 漏掉的其他类型

| 类型 | hal-poc 是否支持 |
|---|---|
| `PackedVector4Array`（format=4 新增） | ❌ |
| `PackedColorArray` | ❌ |
| `PackedInt64Array` / `PackedFloat64Array` | ❌ |
| `RID` | ❌ |
| `Signal` / `Callable`（永远空载入） | ❌ |
| `Object(TypeName, {...})` 内联对象 | ❌ |

源码：`variant_parser.cpp:709-1610`。

## P3 - 节点/场景层补全

### 节点 tag 漏掉的字段

源码：`resource_format_text.cpp:202-281`。

| 字段 | 用途 | hal-poc 是否支持 |
|---|---|---|
| `node_paths` | 延迟应用的 NodePath 属性列表 | ❌ |
| `groups` | 节点分组 | ❌ |
| `instance_placeholder` | 实例占位符 | ❌ |
| `unique_id` | Godot 4.4+ 稳定 ID | ❌ |
| `parent_id_path` / `owner_uid_path` | unique_id 回退路径 | ❌ |

### SceneData 漏掉的字段

| 字段 | 用途 |
|---|---|
| `base_scene` | root 节点带 instance 且无 parent 时，表示场景继承 |
| `editable_instances` | `[editable path="..."]` 段 |

### connection 漏掉的字段

源码：`resource_format_text.cpp:311-390`、`object.h:353-360`。

| 字段 | hal-poc 是否支持 |
|---|---|
| `flags`（默认值 `CONNECT_PERSIST = 2`） | ✅ 但没处理默认值 |
| `binds`（任意 Variant 数组） | ⚠️ 当前是 `Option<Variant>` 应为 `Vec<Variant>` |
| `unbinds` | ❌ |
| `from_uid_path` / `to_uid_path` | ❌ |

ConnectFlags 位含义：
```
CONNECT_DEFERRED          = 1
CONNECT_PERSIST           = 2   // 默认，写入时不输出 flags
CONNECT_ONE_SHOT          = 4
CONNECT_REFERENCE_COUNTED = 8
CONNECT_APPEND_SOURCE_OBJECT = 16
CONNECT_INHERITED         = 32
```

## P4 - 行为对齐

### 错误处理

| 行为 | Godot | hal-poc |
|---|---|---|
| 解析错误 | 立即中止，无恢复 | 一致（pest 默认行为） |
| 未知节点类型 | 创建 MissingNode 或 fallback | 直接报错 |
| 未知属性 key | 静默忽略 | 保留在 props 里 |
| 未知构造器类型 | 报错 `Unexpected identifier` | 降级为 Array |
| ExtResource 文件缺失 | 警告但不致命 | 一致 |

### NodePath 没有裸形式

**我之前误解了**：fixtures 里的 `barepath = "."` 是节点 name 字段（tag 解析），不是 NodePath 值。NodePath 值**必须**写 `NodePath("...")`。源码：`variant_parser.cpp:921-940`。

### StringName 词法层识别

`&"..."` 在词法层就是独立 token（`TK_STRING_NAME`，`variant_parser.cpp:266-276`），不是 parse_value 分发。hal-poc 碰巧用 pest 在 value 层识别，**结果正确但层级不一致**。

## 关键的 FORMAT_VERSION

源码：`resource_format_text.h:42-48`：

```cpp
FORMAT_VERSION = 4,        // 当前版本
FORMAT_VERSION_COMPAT = 3, // 兼容写入版本
```

- **format=2**（Godot 3）：旧类型名（`Quat`/`Matrix3`/`Pool*Array`），数字 ID
- **format=3**（Godot 4.0-4.x 默认）：新类型名，`uid://` 字符串 ID
- **format=4**：新增 base64 PackedByteArray + PackedVector4Array

**hal-poc 应支持 format=3 为主，向前兼容 format=4**。format=2 可选。

注意：VariantParser 本身与 format 版本无关，它一次性支持所有别名。

## 推荐的修复优先级

```
立即做（P0，否则真实 .tscn 解析失败）：
  1. 用真实 Godot 导出 .tscn，验证当前解析器到底失败在哪
  2. 类型化数组 Array[Type]([...])
  3. nil 别名 + 尾逗号
  4. 修正 unescape 未知转义=字面量

短期做（P1，提升覆盖率到 80%）：
  5. inf/-inf/nan
  6. Color #hex 词法
  7. \u \U \b \f 转义

中期做（P2，功能完整）：
  8. 补全几何类型（Vector4/AABB/Transform*/Basis/Quaternion 等）
  9. PackedVector4Array/PackedColorArray 等
  10. 节点 tag 补字段（groups/node_paths/instance_placeholder）

长期做（P3，工业级）：
  11. SceneData 补 base_scene/editable_instances
  12. connection 补 binds/unbinds 完整支持
  13. MissingNode 兼容
```

## 经验教训

1. **手写 fixtures 是危险的** —— 容易写成"我以为"的语法，掩盖真实问题
2. **必须用真实工具的输出做测试** —— Godot 编辑器导出的 .tscn 是黄金测试集
3. **官方源码是最好的文档** —— 比 Godot 文档网站详细 10 倍，且永远是准确的
4. **Variant 类型比预期多很多** —— 19 种 Variant 不够，Godot 有 30+ 种
5. **小细节决定成败** —— 尾逗号、nil 别名这种"小细节"会让真实文件解析失败

## 参考源码路径

- `E:/repos/godot/godot/core/variant/variant_parser.h` — Token 枚举、API
- `E:/repos/godot/godot/core/variant/variant_parser.cpp` — 词法器、parse_value、_parse_array/_parse_dictionary
- `E:/repos/godot/godot/scene/resources/resource_format_text.h` — FORMAT_VERSION
- `E:/repos/godot/godot/scene/resources/resource_format_text.cpp` — 节点/connection tag 解析
- `E:/repos/godot/godot/scene/resources/packed_scene.h` — SceneState/NodeData/ConnectionData 结构
- `E:/repos/godot/godot/scene/resources/packed_scene.cpp` — 实例化、打包、deferred node paths
- `E:/repos/godot/godot/core/object/object.h:353-360` — ConnectFlags
