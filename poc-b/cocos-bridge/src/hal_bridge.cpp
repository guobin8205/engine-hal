// hal_bridge.cpp
//
// POC-B1 的 C++ facade：实现 hal_bridge.h 声明的函数，内部调 Cocos2d-x API。
//
// 架构原则：
// - 全局符号命名空间下定义 hal_xxx 函数（cxxbridge 要求）
// - 用 u64 句柄表达节点，C++ 持有所有权（retain/release）
// - 内部用 std::unordered_map<u64, cocos2d::Node*> 做注册表
// - 异常 try/catch 转 0 返回值（POC 简化错误处理）

#include "hal_facade.h"

#include "cocos2d.h"

#include <atomic>
#include <mutex>
#include <stdexcept>
#include <unordered_map>

// ============================================================
// 节点注册表
// ============================================================

namespace {

std::mutex g_registry_mutex;
std::unordered_map<uint64_t, cocos2d::Node*> g_registry;
std::atomic<uint64_t> g_next_handle{1};

// 注册一个 Node*，返回句柄。内部会 retain（C++ 持有所有权）。
uint64_t register_node(cocos2d::Node* node) {
    if (node == nullptr) {
        return 0;
    }
    node->retain();  // C++ facade 持有一个引用
    uint64_t handle = g_next_handle.fetch_add(1);
    std::lock_guard<std::mutex> lock(g_registry_mutex);
    g_registry[handle] = node;
    return handle;
}

// 取出 Node*（不 release，只查表）。
cocos2d::Node* lookup_node(uint64_t handle) {
    std::lock_guard<std::mutex> lock(g_registry_mutex);
    auto it = g_registry.find(handle);
    return it != g_registry.end() ? it->second : nullptr;
}

// 注销并 release（Rust 的 SpriteHandle::drop 触发）。
void unregister_node(uint64_t handle) {
    cocos2d::Node* node = nullptr;
    {
        std::lock_guard<std::mutex> lock(g_registry_mutex);
        auto it = g_registry.find(handle);
        if (it == g_registry.end()) {
            return;  // 已经被销毁
        }
        node = it->second;
        g_registry.erase(it);
    }
    if (node) {
        node->release();  // 释放 facade 持有的引用
    }
}

}  // namespace

// ============================================================
// facade 函数（cxxbridge 调用）
// ============================================================

// ============ 场景 ============

uint64_t hal_scene_create() {
    cocos2d::Scene* scene = cocos2d::Scene::create();
    return register_node(scene);
}

void hal_director_run_with_scene(uint64_t scene_handle) {
    cocos2d::Node* node = lookup_node(scene_handle);
    cocos2d::Scene* scene = dynamic_cast<cocos2d::Scene*>(node);
    if (scene != nullptr) {
        cocos2d::Director::getInstance()->runWithScene(scene);
    }
}

// ============ 节点通用 ============

void hal_node_destroy(uint64_t handle) {
    unregister_node(handle);
}

void hal_node_set_position(uint64_t handle, float x, float y) {
    cocos2d::Node* node = lookup_node(handle);
    if (node) {
        node->setPosition(cocos2d::Vec2(x, y));
    }
}

void hal_node_add_child(uint64_t parent_handle, uint64_t child_handle) {
    cocos2d::Node* parent = lookup_node(parent_handle);
    cocos2d::Node* child = lookup_node(child_handle);
    if (parent && child) {
        parent->addChild(child);
    }
}

void hal_node_set_visible(uint64_t handle, bool visible) {
    cocos2d::Node* node = lookup_node(handle);
    if (node) {
        node->setVisible(visible);
    }
}

void hal_node_set_color(uint64_t handle, HalColor color) {
    cocos2d::Node* node = lookup_node(handle);
    if (node) {
        // Cocos Color3B 是 0-255 整数，alpha 用 setOpacity
        cocos2d::Color3B c(
            (GLubyte)(color.r * 255.0f),
            (GLubyte)(color.g * 255.0f),
            (GLubyte)(color.b * 255.0f));
        node->setColor(c);
        node->setOpacity((GLubyte)(color.a * 255.0f));
    }
}

// ============ Sprite ============

uint64_t hal_sprite_create(const std::string& texture_path) {
    cocos2d::Sprite* sprite = cocos2d::Sprite::create(texture_path);
    if (sprite == nullptr) {
        return 0;  // 纹理加载失败
    }
    return register_node(sprite);
}

// ============ Label ============

uint64_t hal_label_create(const std::string& text, const std::string& font_path, float size) {
    cocos2d::Label* label = cocos2d::Label::createWithTTF(text, font_path, size);
    if (label == nullptr) {
        // POC 简化：字体失败时 fallback 到系统字体
        label = cocos2d::Label::createWithSystemFont(text, font_path, size);
        if (label == nullptr) {
            return 0;
        }
    }
    return register_node(label);
}

// ============ 调试 ============

size_t hal_node_registry_count() {
    std::lock_guard<std::mutex> lock(g_registry_mutex);
    return g_registry.size();
}
