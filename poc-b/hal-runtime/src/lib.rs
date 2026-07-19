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

        /// 设置节点缩放。
        fn hal_node_set_scale(handle: u64, sx: f32, sy: f32);

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

        // ============ ColorRect ============
        /// 创建纯色矩形（用 LayerColor，用于可视化 Control 布局）。
        fn hal_color_rect_create(width: f32, height: f32, color: HalColor) -> u64;

        // ============ 调试 ============
        /// 返回当前注册的节点数（POC-B1 验证无泄漏用）。
        fn hal_node_registry_count() -> usize;

        // ============ 导出（验证用） ============
        /// 导出 scene 下所有直接子节点的实际坐标/尺寸到 JSON 文件。
        /// 每个 child 用 getTag() 反查 handle，输出 {handle, x, y, w, h}。
        /// 和 Rust 侧的 cocos_export_expected.json 用 handle 关联，
        /// 供 hal-verify 工具对比验证翻译层 + cxx 桥接。
        fn hal_export_scene_nodes(scene: u64, out_path: &CxxString);
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
// C 入口：让 cocos-demo 直接调 Rust
// ============================================================

/// POC-B1 演示入口：创建一个 scene + 一个 sprite，run 起来。
/// （保留作为最小机制验证，不依赖 .tscn 文件）
#[no_mangle]
pub extern "C" fn hal_runtime_run_demo_scene() {
    use cxx::let_cxx_string;

    let scene = ffi::hal_scene_create();
    if scene == 0 {
        return;
    }

    let_cxx_string!(texture = "HelloWorld.png");
    let sprite = ffi::hal_sprite_create(&texture);
    if sprite != 0 {
        ffi::hal_node_set_position(sprite, 480.0, 320.0);
        ffi::hal_node_add_child(scene, sprite);
    }

    ffi::hal_director_run_with_scene(scene);
}

/// POC-B2 端到端入口：从 .tscn 文件构建场景。
///
/// cocos-demo 调这个，传入 .tscn 路径（相对 working directory）。
/// 返回 0 表示失败，非 0 表示场景句柄（已经 run 起来了）。
#[no_mangle]
pub extern "C" fn hal_runtime_run_tscn_scene(tscn_path_ptr: *const u8, len: usize) -> u64 {
    // 把 C 传过来的字符串转成 Rust &str
    if tscn_path_ptr.is_null() || len == 0 {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(tscn_path_ptr, len) };
    let tscn_path = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    match crate::scene_builder::build_scene_from_tscn(tscn_path) {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("POC-B2: 构建 .tscn 场景失败: {}", e);
            // fallback: 用 demo 场景
            hal_runtime_run_demo_scene();
            0
        }
    }
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
