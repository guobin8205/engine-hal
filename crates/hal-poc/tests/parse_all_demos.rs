//! 批量验证测试：对 godot-demo-projects 仓库的所有 .tscn 文件做语义验证。
//!
//! 这些测试验证的不只是"能解析"，而是"解析出的节点数 == 原文件的 [node] tag 数"。
//! 这是真正严格的压力测试。

use hal_poc::parse_scene;
use std::path::PathBuf;
use std::process::Command;

/// godot-demo-projects 仓库路径。
const DEMO_ROOT: &str = "E:/repos/godot/godot-demo-projects";

/// 收集所有 .tscn 文件（递归）。
fn collect_tscn_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_recursive(std::path::Path::new(DEMO_ROOT), &mut files);
    files.sort();
    files
}

fn collect_recursive(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // 跳过 .git 目录
                if path.file_name().and_then(|n| n.to_str()) != Some(".git") {
                    collect_recursive(&path, out);
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("tscn") {
                out.push(path);
            }
        }
    }
}

/// 用 git ls-files 检查 demo 仓库是否存在。
fn demo_repo_available() -> bool {
    std::path::Path::new(DEMO_ROOT).exists()
}

/// 统计原文件中 [node name=...] 的数量（用简单的字符串匹配）。
fn count_node_tags(content: &str) -> usize {
    content.lines().filter(|line| line.starts_with("[node ")).count()
}

/// 统计原文件中 [ext_resource 的数量。
fn count_ext_resources(content: &str) -> usize {
    content
        .lines()
        .filter(|line| line.starts_with("[ext_resource"))
        .count()
}

/// 统计原文件中 [sub_resource 的数量。
fn count_sub_resources(content: &str) -> usize {
    content
        .lines()
        .filter(|line| line.starts_with("[sub_resource"))
        .count()
}

#[test]
fn parse_all_demo_tscn_files_node_count_matches() {
    if !demo_repo_available() {
        eprintln!("跳过：godot-demo-projects 仓库不存在于 {}", DEMO_ROOT);
        return;
    }
    let files = collect_tscn_files();
    assert!(
        files.len() > 100,
        "应至少有 100 个 .tscn 文件，实际 {}",
        files.len()
    );

    let mut mismatches = Vec::new();
    let mut parse_failures = Vec::new();
    let mut total_nodes = 0;

    for path in &files {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                parse_failures.push(format!("读取失败 {}: {}", path.display(), e));
                continue;
            }
        };

        let expected_nodes = count_node_tags(&content);
        match parse_scene(&content) {
            Ok(scene) => {
                total_nodes += scene.nodes.len();
                if scene.nodes.len() != expected_nodes {
                    mismatches.push(format!(
                        "{}: 解析出 {} 节点, 原文件 {} 个 [node] tag",
                        path.display(),
                        scene.nodes.len(),
                        expected_nodes
                    ));
                }
            }
            Err(e) => {
                parse_failures.push(format!("解析失败 {}: {}", path.display(), e));
            }
        }
    }

    println!("=== 批量验证结果 ===");
    println!("总文件数: {}", files.len());
    println!("解析失败: {}", parse_failures.len());
    println!("节点数不匹配: {}", mismatches.len());
    println!("总节点数（解析出）: {}", total_nodes);

    if !parse_failures.is_empty() {
        println!("\n=== 解析失败详情（前 10 个）===");
        for f in parse_failures.iter().take(10) {
            println!("  {}", f);
        }
    }
    if !mismatches.is_empty() {
        println!("\n=== 节点数不匹配详情（前 10 个）===");
        for m in mismatches.iter().take(10) {
            println!("  {}", m);
        }
    }

    assert!(
        parse_failures.is_empty(),
        "有 {} 个文件解析失败",
        parse_failures.len()
    );
    assert!(
        mismatches.is_empty(),
        "有 {} 个文件节点数不匹配",
        mismatches.len()
    );
}

#[test]
fn parse_all_demo_tscn_files_resource_counts_match() {
    if !demo_repo_available() {
        eprintln!("跳过：godot-demo-projects 仓库不存在");
        return;
    }
    let files = collect_tscn_files();

    let mut mismatches = Vec::new();

    for path in &files {
        let content = std::fs::read_to_string(path).unwrap();
        let expected_ext = count_ext_resources(&content);
        let expected_sub = count_sub_resources(&content);

        let scene = parse_scene(&content).expect("解析应在上一测试中验证通过");
        if scene.ext_resources.len() != expected_ext {
            mismatches.push(format!(
                "{}: ext_resource 解析 {} vs 原文件 {}",
                path.display(),
                scene.ext_resources.len(),
                expected_ext
            ));
        }
        if scene.sub_resources.len() != expected_sub {
            mismatches.push(format!(
                "{}: sub_resource 解析 {} vs 原文件 {}",
                path.display(),
                scene.sub_resources.len(),
                expected_sub
            ));
        }
    }

    println!("=== 资源计数验证 ===");
    println!("总文件数: {}", files.len());
    println!("不匹配数: {}", mismatches.len());

    if !mismatches.is_empty() {
        println!("\n=== 不匹配详情（前 10 个）===");
        for m in mismatches.iter().take(10) {
            println!("  {}", m);
        }
    }

    assert!(mismatches.is_empty(), "有 {} 个文件资源数不匹配", mismatches.len());
}

/// 用 dump_tscn 命令行工具跑所有文件，验证 CLI 也能跑通。
/// 这同时验证了库 API 和 dump 输出格式。
#[test]
fn cli_dump_all_demo_tscn_files() {
    if !demo_repo_available() {
        eprintln!("跳过：godot-demo-projects 仓库不存在");
        return;
    }
    // 只在 CI 或显式开启时跑（避免每次 cargo test 都跑 394 个子进程）
    if std::env::var("RUN_CLI_STRESS_TEST").is_err() {
        eprintln!("跳过 CLI 压力测试（设置 RUN_CLI_STRESS_TEST=1 启用）");
        return;
    }

    let files = collect_tscn_files();

    // 查找 exe：cargo workspace 把 target 放在 workspace 根目录
    // 测试的 CARGO_MANIFEST_DIR 是 crates/hal-poc，需要往上一层找 workspace 根
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace 根
        .expect("无法定位 workspace 根");

    let release_exe = workspace_root.join("target/release/examples/dump_tscn.exe");
    let debug_exe = workspace_root.join("target/debug/examples/dump_tscn.exe");
    let exe = if release_exe.exists() {
        release_exe
    } else if debug_exe.exists() {
        debug_exe
    } else {
        panic!(
            "找不到 dump_tscn.exe，请先 cargo build --example dump_tscn\n  尝试过的路径:\n  {}\n  {}",
            release_exe.display(),
            debug_exe.display()
        );
    };
    eprintln!("使用 CLI: {}", exe.display());

    let mut failures = 0;
    let mut exec_errors = 0;

    for path in &files {
        let result = Command::new(&exe).arg(path).output();
        match result {
            Ok(output) => {
                if !output.status.success() {
                    failures += 1;
                    if failures <= 5 {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        println!(
                            "CLI 失败 {}: {}",
                            path.display(),
                            stderr.lines().next().unwrap_or("")
                        );
                    }
                }
            }
            Err(e) => {
                exec_errors += 1;
                if exec_errors <= 3 {
                    println!("无法执行 CLI for {}: {}", path.display(), e);
                }
            }
        }
    }

    println!("=== CLI 压力测试 ===");
    println!("总文件数: {}", files.len());
    println!("解析失败（CLI 退出非 0）: {}", failures);
    println!("执行错误（无法 spawn）: {}", exec_errors);
    assert_eq!(failures, 0, "{} 个文件 CLI 解析失败", failures);
    assert_eq!(exec_errors, 0, "{} 个文件 CLI 无法执行", exec_errors);
}
