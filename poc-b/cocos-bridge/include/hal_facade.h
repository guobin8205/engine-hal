// hal_facade.h
//
// C++ facade 函数声明。由 hal_bridge.cpp 实现，由 cxxbridge 生成的代码调用。
//
// 注意：这个头独立于 cxxbridge 生成的 hal_bridge.h（那个只含 struct 定义）。
// cxx 生成的 generated.cc 调用全局 ::hal_xxx 符号，必须有声明。

#pragma once

#include <cstddef>
#include <cstdint>
#include <string>

#include "hal_bridge.h"  // HalVec2, HalColor struct 定义

// ============ 场景 ============
std::uint64_t hal_scene_create();
void hal_director_run_with_scene(std::uint64_t scene);

// ============ 节点通用 ============
void hal_node_destroy(std::uint64_t handle);
void hal_node_set_position(std::uint64_t handle, float x, float y);
void hal_node_set_scale(std::uint64_t handle, float sx, float sy);
void hal_node_add_child(std::uint64_t parent, std::uint64_t child);
void hal_node_set_visible(std::uint64_t handle, bool visible);
void hal_node_set_color(std::uint64_t handle, HalColor color);

// ============ Sprite / Label ============
std::uint64_t hal_sprite_create(const std::string& texture_path);
std::uint64_t hal_label_create(const std::string& text, const std::string& font_path, float size);
std::uint64_t hal_color_rect_create(float width, float height, HalColor color);

// ============ 调试 ============
std::size_t hal_node_registry_count();
