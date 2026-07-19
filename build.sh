#!/usr/bin/env bash
# POC-B 统一构建脚本
#
# 用法：
#   ./build.sh         完整构建（Rust + C++）
#   ./build.sh run     构建并运行
#
# 约定：Cocos demo 只用 build/ 一个目录（不再 build2/build3/...）

set -e

ENGINE_HAL_ROOT="$(cd "$(dirname "$0")" && pwd)"
COCOS_DEMO="$ENGINE_HAL_ROOT/poc-b/cocos-demo"
HAL_RUNTIME="$ENGINE_HAL_ROOT/poc-b/hal-runtime"
COCOS2DX_ROOT="E:/repos/cocos/cocos2d-x-3.17.2"
CMAKE="C:/Program Files/Microsoft Visual Studio/2022/Professional/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe"

echo "=== 步骤 1: 构建 Rust hal-runtime (32位 Release) ==="
cd "$ENGINE_HAL_ROOT"
cargo build -p hal-runtime --target i686-pc-windows-msvc --release

echo ""
echo "=== 步骤 2: 重新生成 cxx bridge C++ 代码 ==="
cd "$HAL_RUNTIME"
cxxbridge src/lib.rs --header > ../cocos-bridge/include/hal_bridge.h
# 注意：generated.cc 需要手动加 #include "hal_facade.h"（POC 阶段）
cxxbridge src/lib.rs > /tmp/hal_bridge_generated.cc
# 在第 6 行（utility include 后）插入 hal_facade.h
head -5 /tmp/hal_bridge_generated.cc > ../cocos-bridge/src/hal_bridge_generated.cc
echo '' >> ../cocos-bridge/src/hal_bridge_generated.cc
echo '// POC-B patch: 让 generated.cc 看到 facade 函数声明' >> ../cocos-bridge/src/hal_bridge_generated.cc
echo '#include "hal_facade.h"' >> ../cocos-bridge/src/hal_bridge_generated.cc
tail -n +6 /tmp/hal_bridge_generated.cc >> ../cocos-bridge/src/hal_bridge_generated.cc

echo ""
echo "=== 步骤 3: CMake 配置 cocos-demo ==="
mkdir -p "$COCOS_DEMO/build"
cd "$COCOS_DEMO/build"
"$CMAKE" -G "Visual Studio 17 2022" -A Win32 .. > /dev/null

echo ""
echo "=== 步骤 4: 构建 cocos-demo (Release) ==="
"$CMAKE" --build . --config Release

echo ""
echo "=== 步骤 5: 拷贝资源到 exe 目录 ==="
EXE_DIR="$COCOS_DEMO/build/Release"
mkdir -p "$EXE_DIR/Resources"
# 拷贝所有 .tscn 场景文件
cp "$COCOS_DEMO/Resources/"*.tscn "$EXE_DIR/Resources/" 2>/dev/null || true
# Cocos 模板资源（HelloWorld.png / CloseNormal.png 等）
cp "$COCOS2DX_ROOT/templates/cpp-template-default/Resources/"*.png "$EXE_DIR/Resources/" 2>/dev/null || true
cp -r "$COCOS2DX_ROOT/templates/cpp-template-default/Resources/fonts" "$EXE_DIR/Resources/" 2>/dev/null || true

echo ""
echo "✅ 构建完成"
echo "   exe: $EXE_DIR/hal_cocos_demo.exe"
echo "   资源: $EXE_DIR/Resources/"

if [ "$1" = "run" ]; then
    echo ""
    echo "=== 运行 ==="
    cd "$EXE_DIR"
    ./hal_cocos_demo.exe
fi
