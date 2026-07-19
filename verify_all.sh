#!/usr/bin/env bash
# 多场景端到端验证脚本
#
# 用法：
#   ./verify_all.sh   遍历所有测试场景，跑 A(翻译层) + B(端到端) 验证
#
# 前提：已运行过 ./build.sh（生成 exe + hal-verify）
# golden 文件已预生成在 godot_project/<scene>_global.json

set -e

ENGINE_HAL_ROOT="$(cd "$(dirname "$0")" && pwd)"
GODOT_PROJ="$ENGINE_HAL_ROOT/crates/hal-layout/tests/golden/godot_project"
EXE_DIR="$ENGINE_HAL_ROOT/poc-b/cocos-demo/build/Release"
GODOT="E:/tools/godot4.6/Godot_v4.6.2-stable_win64_console.exe"

# 测试场景列表（<name>.tscn 在 Resources/，<name>_global.json 在 godot_project/）
SCENES=(
    "theming_override"
    "pseudolocalization"
    "multiple_resolutions"
    "control_gallery"
)

echo "=========================================="
echo "Engine-HAL 多场景验证（A 翻译层 + B 端到端）"
echo "=========================================="

for scene in "${SCENES[@]}"; do
    echo ""
    echo "=== $scene ==="

    golden="$GODOT_PROJ/${scene}_global.json"

    # 1. 生成全局 golden（如果不存在）
    if [ ! -f "$golden" ]; then
        echo "  生成 golden: $scene"
        cp "$ENGINE_HAL_ROOT/poc-b/cocos-demo/Resources/${scene}.tscn" "$GODOT_PROJ/${scene}.tscn" 2>/dev/null || \
        cp "$GODOT_PROJ/../${scene}.tscn" "$GODOT_PROJ/${scene}.tscn" 2>/dev/null || true
        cd "$GODOT_PROJ"
        "$GODOT" --headless --script export_global_golden.gd -- "${scene}.tscn" "${scene}_global.json" 2>/dev/null | grep "导出" || echo "  (golden 生成失败，跳过 B)"
    fi

    # 2. 跑 Cocos exe（HAL_VERIFY 自动退出）
    cd "$EXE_DIR"
    cp "../../Resources/${scene}.tscn" "Resources/" 2>/dev/null || true
    HAL_VERIFY=1 HAL_SCENE="$scene" ./hal_cocos_demo.exe 2>/dev/null | grep "POC-Export" | head -1

    # 3. 跑 hal-verify 对比
    if [ -f "$golden" ]; then
        cp "$golden" .
        echo "  --- A 翻译层 ---"
        ./hal-verify.exe --golden "${scene}_global.json" 2>/dev/null | grep -E "匹配率|✅|⚠️" | head -2
    else
        echo "  --- A 翻译层（无 golden，只跑 A）---"
        ./hal-verify.exe 2>/dev/null | grep -E "匹配率|✅|⚠️" | head -1
    fi
done

echo ""
echo "=========================================="
echo "完成。"
echo "=========================================="
