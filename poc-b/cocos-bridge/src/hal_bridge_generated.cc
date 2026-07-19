#include <cstddef>
#include <cstdint>
#include <string>
#include <type_traits>
#include <utility>

// POC-B patch: 让 generated.cc 看到 facade 函数声明
#include "hal_facade.h"

#ifdef __GNUC__
#pragma GCC diagnostic ignored "-Wmissing-declarations"
#ifdef __clang__
#pragma clang diagnostic ignored "-Wdollar-in-identifier-extension"
#endif // __clang__
#endif // __GNUC__

#if __cplusplus >= 201402L
#define CXX_DEFAULT_VALUE(value) = value
#else
#define CXX_DEFAULT_VALUE(value)
#endif

struct HalVec2;
struct HalColor;

#ifndef CXXBRIDGE1_STRUCT_HalVec2
#define CXXBRIDGE1_STRUCT_HalVec2
// 共享的 2D 向量类型（POD，按值传）。
struct HalVec2 final {
  float x CXX_DEFAULT_VALUE(0);
  float y CXX_DEFAULT_VALUE(0);

  using IsRelocatable = ::std::true_type;
};
#endif // CXXBRIDGE1_STRUCT_HalVec2

#ifndef CXXBRIDGE1_STRUCT_HalColor
#define CXXBRIDGE1_STRUCT_HalColor
// 共享的颜色类型（POD，按值传）。rgba 0.0-1.0。
struct HalColor final {
  float r CXX_DEFAULT_VALUE(0);
  float g CXX_DEFAULT_VALUE(0);
  float b CXX_DEFAULT_VALUE(0);
  float a CXX_DEFAULT_VALUE(0);

  using IsRelocatable = ::std::true_type;
};
#endif // CXXBRIDGE1_STRUCT_HalColor

extern "C" {
::std::uint64_t cxxbridge1$197$hal_scene_create() noexcept {
  ::std::uint64_t (*hal_scene_create$)() = ::hal_scene_create;
  return hal_scene_create$();
}

void cxxbridge1$197$hal_director_run_with_scene(::std::uint64_t scene) noexcept {
  void (*hal_director_run_with_scene$)(::std::uint64_t) = ::hal_director_run_with_scene;
  hal_director_run_with_scene$(scene);
}

void cxxbridge1$197$hal_node_destroy(::std::uint64_t handle) noexcept {
  void (*hal_node_destroy$)(::std::uint64_t) = ::hal_node_destroy;
  hal_node_destroy$(handle);
}

void cxxbridge1$197$hal_node_set_position(::std::uint64_t handle, float x, float y) noexcept {
  void (*hal_node_set_position$)(::std::uint64_t, float, float) = ::hal_node_set_position;
  hal_node_set_position$(handle, x, y);
}

void cxxbridge1$197$hal_node_set_scale(::std::uint64_t handle, float sx, float sy) noexcept {
  void (*hal_node_set_scale$)(::std::uint64_t, float, float) = ::hal_node_set_scale;
  hal_node_set_scale$(handle, sx, sy);
}

void cxxbridge1$197$hal_node_add_child(::std::uint64_t parent, ::std::uint64_t child) noexcept {
  void (*hal_node_add_child$)(::std::uint64_t, ::std::uint64_t) = ::hal_node_add_child;
  hal_node_add_child$(parent, child);
}

void cxxbridge1$197$hal_node_set_visible(::std::uint64_t handle, bool visible) noexcept {
  void (*hal_node_set_visible$)(::std::uint64_t, bool) = ::hal_node_set_visible;
  hal_node_set_visible$(handle, visible);
}

void cxxbridge1$197$hal_node_set_color(::std::uint64_t handle, ::HalColor *color) noexcept {
  void (*hal_node_set_color$)(::std::uint64_t, ::HalColor) = ::hal_node_set_color;
  hal_node_set_color$(handle, ::std::move(*color));
}

::std::uint64_t cxxbridge1$197$hal_sprite_create(::std::string const &texture_path) noexcept {
  ::std::uint64_t (*hal_sprite_create$)(::std::string const &) = ::hal_sprite_create;
  return hal_sprite_create$(texture_path);
}

::std::uint64_t cxxbridge1$197$hal_label_create(::std::string const &text, ::std::string const &font_path, float size) noexcept {
  ::std::uint64_t (*hal_label_create$)(::std::string const &, ::std::string const &, float) = ::hal_label_create;
  return hal_label_create$(text, font_path, size);
}

::std::size_t cxxbridge1$197$hal_node_registry_count() noexcept {
  ::std::size_t (*hal_node_registry_count$)() = ::hal_node_registry_count;
  return hal_node_registry_count$();
}
} // extern "C"
