//! # hal-runtime
//!
//! POC-B 的 Rust 运行时：通过 cxx 桥接调用 Cocos2d-x C++ facade，
//! 把 Godot .tscn 描述的场景在 Cocos 窗口里显示出来。
//!
//! 架构原则：
//! - C++ 是宿主（Cocos 主循环在 C++）
//! - Rust 通过 `extern "C++"` 调用薄 facade，不直接接触 Cocos 原生 API
//! - 节点用 `u64` 句柄表达，C++ 持有所有权（规避 Ref/autorelease 冲突）

pub mod scene_builder;
pub mod sprite_handle;

pub use sprite_handle::SpriteHandle;

/// cxx 桥接定义。
///
/// `unsafe extern "C++"` 块里的函数由 `cocos-bridge` 的 C++ 侧实现。
/// 所有节点 API 用 u64 句柄，避免直接传 cocos2d::Node*。
///
/// B1 阶段只暴露最小集（scene/sprite/position/destroy），
/// B2 会扩展到 label/node_visible/color 等。
#[cxx::bridge]
pub mod ffi {
    /// 共享的 2D 向量类型（POD，按值传）。
    #[derive(Clone, Copy, Debug)]
    struct HalVec2 {
        pub x: f32,
        pub y: f32,
    }

    /// 共享的颜色类型（POD，按值传）。rgba 0.0-1.0。
    #[derive(Clone, Copy, Debug)]
    struct HalColor {
        pub r: f32,
        pub g: f32,
        pub b: f32,
        pub a: f32,
    }

    unsafe extern "C++" {
        // ============ 场景 ============
        /// 创建一个空场景。返回 u64 句柄。
        fn hal_scene_create() -> u64;

        /// 让 Director 切换到指定场景。
        fn hal_director_run_with_scene(scene: u64);

        // ============ 节点通用 ============
        /// 销毁节点（释放 C++ 侧的 Ref 引用）。
        /// Rust 侧的 SpriteHandle::drop 会调这个。
        fn hal_node_destroy(handle: u64);

        /// 设置节点位置（相对父节点）。
        fn hal_node_set_position(handle: u64, x: f32, y: f32);

        /// 把 child 加到 parent 下。z_order 默认 0。
        fn hal_node_add_child(parent: u64, child: u64);

        /// 设置节点可见性。
        fn hal_node_set_visible(handle: u64, visible: bool);

        /// 设置节点颜色（modulate）。
        fn hal_node_set_color(handle: u64, color: HalColor);

        // ============ Sprite ============
        /// 创建 Sprite，纹理来自 texture_path（相对 Resources 的路径）。
        /// 返回 u64 句柄。失败返回 0（POC 简化错误处理）。
        fn hal_sprite_create(texture_path: &CxxString) -> u64;

        // ============ Label ============
        /// 创建 Label。font_path 是 TTF 路径，size 是字号。
        fn hal_label_create(text: &CxxString, font_path: &CxxString, size: f32) -> u64;

        // ============ 调试 ============
        /// 返回当前注册的节点数（POC-B1 验证无泄漏用）。
        fn hal_node_registry_count() -> usize;
    }
}

impl From<(f32, f32)> for ffi::HalVec2 {
    fn from((x, y): (f32, f32)) -> Self {
        ffi::HalVec2 { x, y }
    }
}

impl ffi::HalVec2 {
    pub fn new(x: f32, y: f32) -> Self {
        ffi::HalVec2 { x, y }
    }
}

impl ffi::HalColor {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        ffi::HalColor { r, g, b, a }
    }

    /// 从 Godot Color (rgba Vec4) 转换。
    pub fn from_godot(c: glam::Vec4) -> Self {
        ffi::HalColor {
            r: c.x,
            g: c.y,
            b: c.z,
            a: c.w,
        }
    }
}

// ============================================================
// C 入口（POC-B1 简化版）：让 cocos-demo 直接调 Rust
// ============================================================

/// POC-B1 演示入口：创建一个 scene + 一个 sprite，run 起来。
///
/// cocos-demo 的 AppDelegate::applicationDidFinishLaunching 调这个。
/// 内部通过 cxx bridge 调 C++ facade。
///
/// 注意：这是个 C ABI 函数（extern "C"），不走 cxx bridge。
/// cxx bridge 用于 Rust → C++ 方向（调 facade）。
/// C++ → Rust 方向我们用简单 extern "C"（POC 简化）。
#[no_mangle]
pub extern "C" fn hal_runtime_run_demo_scene() {
    use cxx::let_cxx_string;

    // 1. 创建场景
    let scene = ffi::hal_scene_create();
    if scene == 0 {
        return;
    }

    // 2. 创建一个 sprite（用 Cocos 内置的 HelloWorld.png 占位）
    //    POC-B1 只验证机制，B2 才真正解析 .tscn
    let_cxx_string!(texture = "HelloWorld.png");
    let sprite = ffi::hal_sprite_create(&texture);
    if sprite != 0 {
        // 居中显示
        ffi::hal_node_set_position(sprite, 480.0, 320.0);
        ffi::hal_node_add_child(scene, sprite);
        // 注意：不调 hal_node_destroy —— sprite 由 Cocos scene graph 管理
        // （POC-B1 验证：facade retain 后，Cocos addChild 也会 retain，引用计数正确）
    }

    // 3. 切换到这个场景
    ffi::hal_director_run_with_scene(scene);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halvec2_construct() {
        let v = ffi::HalVec2::new(1.0, 2.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
    }

    #[test]
    fn halcolor_from_godot() {
        let c = ffi::HalColor::from_godot(glam::Vec4::new(1.0, 0.0, 0.5, 1.0));
        assert_eq!(c.r, 1.0);
        assert_eq!(c.b, 0.5);
    }
}
