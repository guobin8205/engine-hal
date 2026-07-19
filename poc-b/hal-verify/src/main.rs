//! hal-verify: Cocos 显示正确性验证（翻译层 + 端到端）。
//!
//! **A. 翻译层**：expected Cocos 坐标 vs actual Cocos 坐标（容差 1px）
//!   验证 Godot 全局坐标 → Y 翻转 → LayerColor anchor → cxx 桥接 → setPosition
//!
//! **B. 端到端**：actual 翻回 Godot 全局坐标 vs Godot 全局 golden（容差 5px）
//!   验证 .tscn → hal-layout 计算 → Cocos 显示的全链路，和 Godot 真实渲染对比。
//!   需要先用 export_global_golden.gd 生成全局 golden。
//!
//! 用法：
//!   hal-verify [--expected <p>] [--actual <p>] [--exe-dir <dir>] [--golden <global_golden.json>]

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

const WINDOW_HEIGHT: f64 = 640.0;
const TOLERANCE_TRANSLATE: f64 = 1.0;
const TOLERANCE_END_TO_END: f64 = 5.0;

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

#[derive(Debug, Deserialize)]
struct ActualEntry {
    handle: u64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Debug, Deserialize)]
struct GoldenNode {
    #[allow(dead_code)]
    name: String,
    gx: f64,
    gy: f64,
    w: f64,
    h: f64,
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cfg = parse_args(&args);

    let expected = load_json::<Vec<ExpectedEntry>>(&cfg.expected, "expected");
    let actual = load_json::<Vec<ActualEntry>>(&cfg.actual, "actual");
    let golden = cfg.golden.as_ref().and_then(|p| load_json_opt::<HashMap<String, GoldenNode>>(p, "golden"));

    println!("=== hal-verify: Cocos 显示正确性验证 ===");
    println!("expected: {} ({} ColorRect)", cfg.expected.display(), expected.len());
    println!("actual:   {} ({} ColorRect)", cfg.actual.display(), actual.len());
    if let Some(ref g) = golden {
        println!("golden:   {} ({} 节点)", cfg.golden.as_ref().unwrap().display(), g.len());
    }
    println!();

    let a_ok = verify_translate(&expected, &actual);
    let b_ok = if let Some(ref g) = golden {
        verify_end_to_end(&expected, &actual, g)
    } else {
        println!("\n[B 端到端] 跳过（未指定 --golden）");
        true
    };

    if a_ok && b_ok {
        println!("\n✅ 全部通过");
        std::process::ExitCode::SUCCESS
    } else {
        println!("\n❌ 存在不匹配，详见上方报告");
        std::process::ExitCode::FAILURE
    }
}

fn verify_translate(expected: &[ExpectedEntry], actual: &[ActualEntry]) -> bool {
    println!("[A 翻译层] expected Cocos 坐标 vs actual（容差 {}px）", TOLERANCE_TRANSLATE);
    let actual_by_handle: HashMap<u64, &ActualEntry> = actual.iter().map(|a| (a.handle, a)).collect();

    let mut m = 0usize;
    let mut miss = 0usize;
    let mut nf = 0usize;
    let mut details = Vec::new();
    for e in expected {
        match actual_by_handle.get(&e.handle) {
            None => { nf += 1; }
            Some(a) => {
                let dx = (e.cocos_x - a.x).abs();
                let dy = (e.cocos_y - a.y).abs();
                let dw = (e.w - a.w).abs();
                let dh = (e.h - a.h).abs();
                if dx <= TOLERANCE_TRANSLATE && dy <= TOLERANCE_TRANSLATE && dw <= TOLERANCE_TRANSLATE && dh <= TOLERANCE_TRANSLATE {
                    m += 1;
                } else {
                    miss += 1;
                    if miss <= 20 {
                        details.push(format!(
                            "  ❌ {}: expected=({:.0},{:.0},{:.0},{:.0}) actual=({:.0},{:.0},{:.0},{:.0}) dx={:.0} dy={:.0} dw={:.0} dh={:.0}",
                            e.path, e.cocos_x, e.cocos_y, e.w, e.h, a.x, a.y, a.w, a.h, dx, dy, dw, dh
                        ));
                    }
                }
            }
        }
    }

    print_report("翻译层", m, miss, nf, &details);
    miss == 0 && nf == 0
}

fn verify_end_to_end(
    expected: &[ExpectedEntry],
    actual: &[ActualEntry],
    golden: &HashMap<String, GoldenNode>,
) -> bool {
    println!("\n[B 端到端] Cocos actual 翻回 Godot 全局 vs Godot golden（容差 {}px）", TOLERANCE_END_TO_END);
    let actual_by_handle: HashMap<u64, &ActualEntry> = actual.iter().map(|a| (a.handle, a)).collect();

    let mut m = 0usize;
    let mut miss = 0usize;
    let mut details = Vec::new();
    for e in expected {
        let Some(a) = actual_by_handle.get(&e.handle) else { continue };
        let Some(g) = golden.get(&e.path) else { continue };

        // actual (Cocos) → Godot 全局
        let godot_x = a.x;
        let godot_y = WINDOW_HEIGHT - a.y - a.h;
        let dx = (godot_x - g.gx).abs();
        let dy = (godot_y - g.gy).abs();
        let dw = (a.w - g.w).abs();
        let dh = (a.h - g.h).abs();
        if dx <= TOLERANCE_END_TO_END && dy <= TOLERANCE_END_TO_END && dw <= TOLERANCE_END_TO_END && dh <= TOLERANCE_END_TO_END {
            m += 1;
        } else {
            miss += 1;
            if miss <= 20 {
                details.push(format!(
                    "  ❌ {}: cocos_actual→godot=({:.0},{:.0},{:.0},{:.0}) golden=({:.0},{:.0},{:.0},{:.0}) dx={:.0} dy={:.0} dw={:.0} dh={:.0}",
                    e.path, godot_x, godot_y, a.w, a.h, g.gx, g.gy, g.w, g.h, dx, dy, dw, dh
                ));
            }
        }
    }

    let compared = m + miss;
    let rate = if compared > 0 { m as f64 / compared as f64 * 100.0 } else { 0.0 };
    println!("  对比: {} (匹配 {} + 不匹配 {})", compared, m, miss);
    println!("  匹配率: {:.1}% ({}/{})", rate, m, compared);
    for d in &details {
        println!("{}", d);
    }
    if rate >= 95.0 {
        println!("  ✅ 端到端匹配率 ≥ 95%");
        true
    } else {
        println!("  ⚠️ 端到端匹配率 < 95%");
        false
    }
}

fn print_report(label: &str, m: usize, miss: usize, nf: usize, details: &[String]) {
    let compared = m + miss;
    let rate = if compared > 0 { m as f64 / compared as f64 * 100.0 } else { 0.0 };
    println!("  对比: {} (匹配 {} + 不匹配 {})", compared, m, miss);
    if nf > 0 { println!("  actual 缺失: {}", nf); }
    println!("  匹配率: {:.1}% ({}/{})", rate, m, compared);
    for d in details {
        println!("{}", d);
    }
    if miss == 0 && nf == 0 {
        println!("  ✅ {} 精确匹配", label);
    } else {
        println!("  ⚠️ {} 存在偏差", label);
    }
}

struct Config {
    expected: PathBuf,
    actual: PathBuf,
    golden: Option<PathBuf>,
}

fn parse_args(args: &[String]) -> Config {
    let mut expected = None;
    let mut actual = None;
    let mut golden = None;
    let mut exe_dir = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--expected" if i + 1 < args.len() => { expected = Some(args[i + 1].clone()); i += 2; }
            "--actual" if i + 1 < args.len() => { actual = Some(args[i + 1].clone()); i += 2; }
            "--golden" if i + 1 < args.len() => { golden = Some(args[i + 1].clone()); i += 2; }
            "--exe-dir" if i + 1 < args.len() => { exe_dir = Some(args[i + 1].clone()); i += 2; }
            _ => { i += 1; }
        }
    }
    let dir: PathBuf = exe_dir.map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_exe().ok()
            .and_then(|p| p.parent().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(".")));
    Config {
        expected: expected.map(PathBuf::from).unwrap_or_else(|| dir.join("cocos_export_expected.json")),
        actual: actual.map(PathBuf::from).unwrap_or_else(|| dir.join("cocos_export_actual.json")),
        golden: golden.map(PathBuf::from),
    }
}

fn load_json<T: serde::de::DeserializeOwned>(path: &std::path::Path, label: &str) -> T {
    load_json_opt(path, label).unwrap_or_else(|| {
        eprintln!("错误：无法读取 {} 文件: {}", label, path.display());
        std::process::exit(1);
    })
}

fn load_json_opt<T: serde::de::DeserializeOwned>(path: &std::path::Path, label: &str) -> Option<T> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).map_err(|e| {
        eprintln!("警告：解析 {} 失败 ({}): {}", label, path.display(), e);
        e
    }).ok()
}
