# Golden Test: 用 Godot 验证 hal-layout 布局正确性

## 原理

Godot 是布局正确性的"黄金标准"。我们：
1. 让 Godot 加载测试场景，导出每个 Control 的最终 position+size
2. 让 hal-layout 算同样的场景
3. 对比两者结果，差值应该 < 1 像素

## 用法

### 1. 用 Godot 导出 golden 数据

```bash
# 用 Godot headless 模式运行（不需要打开编辑器）
godot --headless --path . layout_test.tscn
```

Godot 会输出 JSON 到：
- 控制台
- `%USERPROFILE%/AppData/Roaming/Godot/app_userdata/layout_golden.json`

把导出的 JSON 复制到 `layout_golden.json`（本目录）。

### 2. 跑 Rust golden test

```bash
cargo test -p hal-layout golden_test -- --nocapture
```

Rust 测试会：
- 解析 `layout_test.tscn`
- 用 hal-layout 算布局
- 和 `layout_golden.json` 对比
- 差值 < 1 像素的算通过

## 测试场景覆盖

`layout_test.tscn` 覆盖的布局 case：

| 节点名 | 锚点配置 | 验证点 |
|---|---|---|
| TopLeft | TopLeft preset, offset 10/20/110/70 | 固定大小 + 左上角 |
| Center | Center preset, offset -50/-25/50/25 | 居中固定大小 |
| FullRect | FullRect preset, offset 5/-5 | 响应式拉伸（去掉边距） |
| BottomWide | BottomWide preset, offset -100/0 | 底部全宽 |
| VBox | LeftWide preset | 容器自身锚点 |
| VItem1/2/3 | VBox 子节点 | VBox 垂直排列（50/30/40 高度） |
