// ═══════════════════════════════════════════════════════════════
// opt_analyze — Optick .opt 性能分析器 (Rust 版)
// 零外部依赖，纯 std 实现
//
// 用法: opt_analyze [文件.opt] [慢帧阈值ms]
// 示例: opt_analyze "capture.opt" 100
// ═══════════════════════════════════════════════════════════════

mod binary;
mod model;
mod parser;
mod call_tree;
mod stats;
mod analyzer;
mod report;

use std::env;
use std::fs;
use std::time::Instant;

struct Args {
    input: String,
    threshold: f64,
}

fn parse_args() -> Args {
    let args: Vec<String> = env::args().collect();
    let input = args.get(1).cloned().unwrap_or_else(|| "capture.opt".into());
    let threshold: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100.0);
    Args { input, threshold }
}

fn main() {
    let args = parse_args();

    let t0 = Instant::now();
    let buf = fs::read(&args.input).expect("无法读取文件");
    let t_read = t0.elapsed();

    let t1 = Instant::now();
    let mut data = parser::parse(&buf);
    let t_parse = t1.elapsed();

    let t2 = Instant::now();
    call_tree::build_call_trees(&mut data.frames);
    let t_tree = t2.elapsed();

    let t3 = Instant::now();
    let result = analyzer::analyze(&data, args.threshold);
    let t_analyze = t3.elapsed();

    report::print_report(&data, &result, &args.input);

    let total = t0.elapsed();
    eprintln!("\n── 耗时统计 ──");
    eprintln!("  读取: {:.1} ms | 解析: {:.1} ms | 调用树: {:.1} ms | 分析: {:.1} ms | 总计: {:.1} ms",
        t_read.as_secs_f64() * 1000.0,
        t_parse.as_secs_f64() * 1000.0,
        t_tree.as_secs_f64() * 1000.0,
        t_analyze.as_secs_f64() * 1000.0,
        total.as_secs_f64() * 1000.0,
    );
}