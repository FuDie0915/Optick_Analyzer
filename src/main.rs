// ═══════════════════════════════════════════════════════════════
// opt_analyze — Optick .opt 性能分析器 (Rust 版)
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
use std::io::Read;
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

/// 读取 .opt 文件，检测 gzip 压缩并自动解压
/// 返回 (解压后的完整字节, 原始文件大小)
fn read_opt(path: &str) -> (Vec<u8>, f64) {
    let raw = fs::read(path).expect("无法读取文件");
    let file_size_mb = raw.len() as f64 / 1_048_576.0;

    // 文件头: magic(4) + version(2) + flags(2)
    if raw.len() < 8 || u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) != 0xB50FB50F {
        eprintln!("无效 Optick 文件");
        std::process::exit(1);
    }

    let flags = u16::from_le_bytes([raw[6], raw[7]]);

    // flags bit0: gzip 压缩; bit1: miniz/zlib 压缩
    if flags & 1 != 0 {
        // gzip: 跳过 8 字节 Optick 头，剩余是 gzip 流
        let mut decoder = flate2::read::GzDecoder::new(&raw[8..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).expect("gzip 解压失败");

        // 重建: 8 字节原始头 + 解压后的块数据
        let mut buf = Vec::with_capacity(8 + decompressed.len());
        buf.extend_from_slice(&raw[..8]);
        // 清除压缩标志
        buf[6] = 0;
        buf[7] = 0;
        buf.extend_from_slice(&decompressed);
        (buf, file_size_mb)
    } else if flags & 2 != 0 {
        // miniz/zlib: 跳过 8 字节头，剩余是 zlib 流
        let mut decoder = flate2::read::ZlibDecoder::new(&raw[8..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).expect("zlib 解压失败");

        let mut buf = Vec::with_capacity(8 + decompressed.len());
        buf.extend_from_slice(&raw[..8]);
        buf[6] = 0;
        buf[7] = 0;
        buf.extend_from_slice(&decompressed);
        (buf, file_size_mb)
    } else {
        (raw, file_size_mb)
    }
}

fn main() {
    let args = parse_args();

    let t0 = Instant::now();
    let (buf, file_size_mb) = read_opt(&args.input);
    let t_read = t0.elapsed();

    let t1 = Instant::now();
    let mut data = parser::parse(&buf, file_size_mb);
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