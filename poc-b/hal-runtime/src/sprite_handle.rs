//! Rust 端的节点句柄 RAII 包装。
//!
//! `SpriteHandle` 持有 u64 句柄，Drop 时自动调 `hal_node_destroy`
//! 让 C++ 侧 release 掉对应的 cocos2d::Sprite。
//!
//! 这是 cxx + Cocos Ref 安全桥接的核心：
//! - Rust 完全不感知 cocos2d::Ref
//! - 所有权通过 Rust 的 Drop 语义对齐 C++ 的 release()
//! - 句柄是 u64，可以序列化、跨线程传递（C++ facade 自己保证线程安全）

use cxx::let_cxx_string;

use crate::ffi;

/// Sprite 节点的 Rust 句柄。
///
/// Drop 时会调 C++ facade 的 `hal_node_destroy` 释放引用。
pub struct SpriteHandle {
    handle: u64,
}

impl SpriteHandle {
    /// 通过 C++ facade 创建 Sprite。texture_path 相对 Resources 目录。
    ///
    /// 失败时返回 None（facade 返回 0）。
    pub fn create(texture_path: &str) -> Option<Self> {
        let_cxx_string!(c_path = texture_path);
        let handle = ffi::hal_sprite_create(&c_path);
        if handle == 0 {
            None
        } else {
            Some(SpriteHandle { handle })
        }
    }

    /// 获取底层 u64 句柄（用于调 facade API）。
    pub fn handle(&self) -> u64 {
        self.handle
    }

    /// 设置位置。
    pub fn set_position(&self, x: f32, y: f32) {
        ffi::hal_node_set_position(self.handle, x, y);
    }

    /// 设置可见性。
    pub fn set_visible(&self, visible: bool) {
        ffi::hal_node_set_visible(self.handle, visible);
    }

    /// 设置颜色（modulate）。
    pub fn set_color(&self, color: ffi::HalColor) {
        ffi::hal_node_set_color(self.handle, color);
    }

    /// 加到父节点下。
    pub fn add_to_parent(&self, parent: u64) {
        ffi::hal_node_add_child(parent, self.handle);
    }
}

impl Drop for SpriteHandle {
    fn drop(&mut self) {
        if self.handle != 0 {
            ffi::hal_node_destroy(self.handle);
            self.handle = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_handle_zero_is_invalid() {
        // 0 表示无效句柄（facade 失败时返回 0）
        let h = SpriteHandle { handle: 0 };
        assert_eq!(h.handle(), 0);
    }
}

