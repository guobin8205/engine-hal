//! 命令行工具：dump 一个 .tscn 文件的可读内容。
//!
//! 用法：
//!   cargo run --example dump_tscn -- path/to/scene.tscn
//!   cargo run --example dump_tscn -- --json path/to/scene.tscn
//!
//! 不带参数时，dump 一个内置 fixture。

use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut json_mode = false;
    let mut path_arg: Option<String> = None;

    for arg in args.iter().skip(1) {
        if arg == "--json" {
            json_mode = true;
        } else if arg == "-h" || arg == "--help" {
            print_usage(&args[0]);
            return;
        } else {
            path_arg = Some(arg.clone());
        }
    }

    let path = match &path_arg {
        Some(p) => p.clone(),
        None => {
            // 默认 dump sprite.tscn fixture
            let p = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("sprite.tscn");
            p.to_string_lossy().into_owned()
        }
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("无法读取 {}: {}", path, e);
            std::process::exit(1);
        }
    };

    match hal_poc::parse_scene(&content) {
        Ok(scene) => {
            if json_mode {
                match serde_json::to_string_pretty(&scene) {
                    Ok(json) => println!("{}", json),
                    Err(e) => {
                        eprintln!("JSON 序列化失败: {}", e);
                        std::process::exit(2);
                    }
                }
            } else {
                print!("{}", hal_poc::dump::dump_scene(&scene));
            }
        }
        Err(e) => {
            eprintln!("解析失败: {}", e);
            std::process::exit(3);
        }
    }
}

fn print_usage(prog: &str) {
    eprintln!("用法: {} [--json] [<path-to-tscn>]", prog);
    eprintln!();
    eprintln!("选项:");
    eprintln!("  --json    以 JSON 格式输出（便于程序消费）");
    eprintln!("  -h, --help  显示帮助");
    eprintln!();
    eprintln!("不带参数时默认 dump sprite.tscn fixture。");
}
