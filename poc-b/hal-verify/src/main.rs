//! hal-verify: 对比 Rust 期望坐标 vs Cocos 实际渲染坐标，验证翻译层正确性。
//!
//! **验证内容**：Rust hal-layout 算的 ComputedLayout（Godot 坐标）→ Cocos 坐标转换
//! （Y 轴翻转 + LayerColor anchor）→ cxx 桥接 → Cocos 实际 setPosition/getPosition。
//!
//! **数据流**：
//!   - cocos_export_expected.json（Rust 导出）：每个 ColorRect 占位符的 path/handle/
//!     期望 Cocos 坐标（= Godot 全局坐标 Y 翻转后）
//!   - cocos_export_actual.json（C++ 导出）：scene 下每个 ColorRect 的 handle/实际 Cocos 坐标
//!   - 用 handle 关联，对比期望 vs 实际（容差 1px，坐标转换是纯数学无精度损失）
//!
//! **只对比 ColorRect 占位符**（不对比 Sprite/Label）：
//!   ColorRect 的 size 来自 hal-layout（反映布局），而 Sprite/Label 的 size 来自
//!   Cocos 字体/纹理渲染，和 hal-layout 不一致，对比它们没意义。
//!
//! **坐标系说明**：
//!   expected 和 actual 都是 Cocos 全局坐标（scene root 为原点）。
//!   Rust 算的 Godot 全局坐标正确性由 hal-layout 的 gallery_golden_test.rs 验证
//!   （对比 Godot position+size），本工具只验证 Godot→Cocos 的翻译层。
//!
//! 用法：
//!   hal-verify [--expected <path>] [--actual <path>] [--exe-dir <dir>]

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

const TOLERANCE: f64 = 1.0; // 坐标转换是纯数学，无精度损失，严格 1px

/// Rust 侧导出的期望记录（和 hal-runtime::ExpectedEntry 对齐）
#[derive(Debug, Deserialize)]
struct ExpectedEntry {
    path: String,
    handle: u64,
    #[allow(dead_code)]
    godot_x: f64,
    #[allow(dead_code)]
    godot_y: f64,
    w: f64,
    h: f64,
    cocos_x: f64,
    cocos_y: f64,
}

/// C++ 侧导出的实际记录
#[derive(Debug, Deserialize)]
struct ActualEntry {
    handle: u64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (expected_path, actual_path) = parse_args(&args);

    let expected = load_json::<Vec<ExpectedEntry>>(&expected_path, "expected");
    let actual = load_json::<Vec<ActualEntry>>(&actual_path, "actual");

    println!("=== hal-verify: Cocos 显示翻译层验证 ===");
    println!("expected: {} ({} 个 ColorRect 占位符)", expected_path.display(), expected.len());
    println!("actual:   {} ({} 个 ColorRect)", actual_path.display(), actual.len());
    println!("容差: {}px", TOLERANCE);
    println!();

    let actual_by_handle: HashMap<u64, &ActualEntry> = actual.iter().map(|a| (a.handle, a)).collect();

    let mut match_count = 0usize;
    let mut miss_count = 0usize;
    let mut not_found = 0usize;
    let mut details: Vec<String> = Vec::new();

    for e in &expected {
        match actual_by_handle.get(&e.handle) {
            None => {
                not_found += 1;
                if not_found <= 5 {
                    println!("  ⚠️ {} (handle={}): actual 缺失", e.path, e.handle);
                }
            }
            Some(a) => {
                let dx = (e.cocos_x - a.x).abs();
                let dy = (e.cocos_y - a.y).abs();
                let dw = (e.w - a.w).abs();
                let dh = (e.h - a.h).abs();
                if dx <= TOLERANCE && dy <= TOLERANCE && dw <= TOLERANCE && dh <= TOLERANCE {
                    match_count += 1;
                } else {
                    miss_count += 1;
                    if miss_count <= 20 {
                        details.push(format!(
                            "  ❌ {}: expected=({:.0},{:.0},{:.0},{:.0}) actual=({:.0},{:.0},{:.0},{:.0}) dx={:.0} dy={:.0} dw={:.0} dh={:.0}",
                            e.path, e.cocos_x, e.cocos_y, e.w, e.h, a.x, a.y, a.w, a.h, dx, dy, dw, dh
                        ));
                    }
                }
            }
        }
    }

    let compared = match_count + miss_count;
    let rate = if compared > 0 { match_count as f64 / compared as f64 * 100.0 } else { 0.0 };

    println!("对比: {} (匹配 {} + 不匹配 {})", compared, match_count, miss_count);
    if not_found > 0 {
        println!("actual 缺失: {}", not_found);
    }
    println!("匹配率: {:.1}% ({}/{})", rate, match_count, compared);

    if !details.is_empty() {
        println!("\n不匹配详情:");
        for d in &details {
            println!("{}", d);
        }
    }

    if miss_count == 0 && not_found == 0 {
        println!("\n✅ 翻译层精确匹配（expected Cocos 坐标 == actual Cocos 坐标）");
        println!("   Godot 全局坐标 → Y 轴翻转 → LayerColor anchor → cxx 桥接 → setPosition 全链路正确");
        std::process::ExitCode::SUCCESS
    } else {
        println!("\n❌ 翻译层存在偏差（详见上方）");
        std::process::ExitCode::FAILURE
    }
}

fn parse_args(args: &[String]) -> (PathBuf, PathBuf) {
    let mut expected = None;
    let mut actual = None;
    let mut exe_dir = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--expected" if i + 1 < args.len() => { expected = Some(args[i + 1].clone()); i += 2; }
            "--actual" if i + 1 < args.len() => { actual = Some(args[i + 1].clone()); i += 2; }
            "--exe-dir" if i + 1 < args.len() => { exe_dir = Some(args[i + 1].clone()); i += 2; }
            _ => { i += 1; }
        }
    }

    let dir: PathBuf = exe_dir.map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_exe().ok()
            .and_then(|p| p.parent().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(".")));

    (
        expected.map(PathBuf::from).unwrap_or_else(|| dir.join("cocos_export_expected.json")),
        actual.map(PathBuf::from).unwrap_or_else(|| dir.join("cocos_export_actual.json")),
    )
}

fn load_json<T: serde::de::DeserializeOwned>(path: &std::path::Path, label: &str) -> T {
    let content = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("错误：无法读取 {} 文件 ({}): {}", label, path.display(), e);
        std::process::exit(1);
    });
    serde_json::from_str(&content).unwrap_or_else(|e| {
        eprintln!("错误：解析 {} 失败 ({}): {}", label, path.display(), e);
        std::process::exit(1);
    })
}
