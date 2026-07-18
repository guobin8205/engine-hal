#pragma once
#include <cstddef>
#include <cstdint>
#include <string>
#include <type_traits>

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
