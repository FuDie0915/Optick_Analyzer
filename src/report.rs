//! 报告输出

use std::collections::{HashMap, HashSet};
use crate::model::*;
use crate::stats;

pub fn print_report(data: &ParsedData, result: &AnalysisResult, input: &str) {
    print_header(result);
    print_file_info(data, result, input);
    print_frame_stats(result);
    print_frame_budget(result);
    print_pareto(result);
    print_thread_analysis(data, result);
    print_frame_timeline(data, result);
    print_slow_frame_details(data, result);
    print_cross_frame_hotspots(result);
    print_module_aggregation(result);
    print_call_frequency(result);
    print_stability(result);
    print_caller_callee(data, result);
    print_critical_path(result);
    print_suggestions(data, result);
}

// ── 各报告段落 ──

fn print_header(result: &AnalysisResult) {
    println!("══════════════════════════════════════════════════════════════");
    println!("  Optick 性能分析报告 v2.0 (Rust)");
    println!("══════════════════════════════════════════════════════════════\n");

    if let Some(top1) = result.all_funcs.first() {
        let pct = if result.total_slow_ms > 0.0 { top1.total_self / result.total_slow_ms * 100.0 } else { 0.0 };
        println!("── 核心结论 ──────────────────────────────────────────────────");
        println!("  瓶颈函数: {} ({})", top1.name, stats::file_short(&top1.file));
        println!("  独占 {:.1}% 卡顿时间 ({:.0} ms / {:.0} ms)", pct, top1.total_self, result.total_slow_ms);
        println!("  出现在 {}/{} 卡顿帧，单帧最高 {:.0} ms\n", top1.frame_count, result.slow_count, top1.max_self);
    }
}

fn print_file_info(data: &ParsedData, result: &AnalysisResult, input: &str) {
    println!("── 文件信息 ──────────────────────────────────────────────────");
    println!("  文件: {}", input);
    println!("  大小: {:.1} MB | 时钟: {:.1} MHz", data.file_size_mb, data.frequency as f64 / 1e6);
    println!("  线程: {} | 事件描述: {} | 总帧数: {} | 总事件: {}\n",
        data.threads.len(), data.evt_count, result.frame_count, result.total_events);
}

fn print_frame_stats(result: &AnalysisResult) {
    println!("── 帧耗时统计 ────────────────────────────────────────────────");
    println!("  最快     P25      P50      P75      P90      P95      P99      最慢     均值     标准差");
    println!("  {:>7.1}  {:>7.1}  {:>7.1}  {:>7.1}  {:>7.1}  {:>7.1}  {:>7.1}  {:>7.1}  {:>7.1}  {:>7.1}\n",
        result.min_ms, stats::pct(&result.sorted_dur, 0.25), stats::pct(&result.sorted_dur, 0.50), stats::pct(&result.sorted_dur, 0.75),
        stats::pct(&result.sorted_dur, 0.90), stats::pct(&result.sorted_dur, 0.95), stats::pct(&result.sorted_dur, 0.99),
        result.max_ms, result.mean_ms, result.std_ms);
}

fn print_frame_budget(result: &AnalysisResult) {
    println!("── 帧预算分析 ────────────────────────────────────────────────");
    println!("  60fps 预算 (16.7ms): {}/{} 帧超标 ({:.1}%)", result.over_60fps, result.frame_count, result.over_60fps as f64 / result.frame_count as f64 * 100.0);
    println!("  30fps 预算 (33.3ms): {}/{} 帧超标 ({:.1}%)", result.over_30fps, result.frame_count, result.over_30fps as f64 / result.frame_count as f64 * 100.0);
    println!("  自定义阈值 ({:.0}ms): {}/{} 帧超标 ({:.1}%)\n", result.threshold, result.slow_count, result.frame_count, result.slow_count as f64 / result.frame_count as f64 * 100.0);
}

fn print_pareto(result: &AnalysisResult) {
    println!("── 热点集中度分析 (Pareto) ────────────────────────────────────");
    if result.total_slow_ms > 0.0 {
        println!("  Top  1 函数: {:>10.1} ms ({:.1}%)", result.top1_self, result.top1_self / result.total_slow_ms * 100.0);
        println!("  Top  3 函数: {:>10.1} ms ({:.1}%)", result.top3_self, result.top3_self / result.total_slow_ms * 100.0);
        println!("  Top  5 函数: {:>10.1} ms ({:.1}%)", result.top5_self, result.top5_self / result.total_slow_ms * 100.0);
        println!("  Top 10 函数: {:>10.1} ms ({:.1}%)\n", result.top10_self, result.top10_self / result.total_slow_ms * 100.0);
    }
}

fn print_thread_analysis(data: &ParsedData, result: &AnalysisResult) {
    println!("── 线程分析 (按总耗时降序) ───────────────────────────────────");
    println!("  线程名              帧数   总耗时(ms)  事件数    占比");
    for (name, count, total_ms, events) in &result.thread_stats {
        let pct = if result.total_slow_ms > 0.0 { total_ms / result.total_slow_ms * 100.0 } else { 0.0 };
        let is_slow = result.slow_indices.iter().any(|&i| data.frames[i].thread_name == *name);
        if is_slow {
            println!("  {:<18} {:>4}   {:>10.1}   {:>8}   {:>5.1}%", name, count, total_ms, events, pct);
        }
    }
    println!();
}

fn print_frame_timeline(data: &ParsedData, result: &AnalysisResult) {
    println!("── 帧时间线 (按捕获顺序) ─────────────────────────────────────");
    println!("  序号  线程            耗时(ms)    Δ前帧    事件数   可视化               状态");
    let mut prev_ms = 0.0f64;
    for (i, f) in data.frames.iter().enumerate() {
        let dur = result.durations[i];
        let delta = if i > 0 { dur - prev_ms } else { 0.0 };
        prev_ms = dur;
        let status = if dur >= result.threshold { "卡顿" } else if dur > 33.33 { "30fps" } else if dur > 16.67 { "60fps" } else { "流畅" };
        let delta_str = if i > 0 { format!("{:+.1}", delta) } else { "-".into() };
        println!("  [{:>2}]  {:<14} {:>8.1}  {:>8}  {:>8}   {}  {}",
            i, f.thread_name, dur, delta_str, f.events.len(), stats::bar(dur), status);
    }
    println!();
}

fn print_slow_frame_details(data: &ParsedData, result: &AnalysisResult) {
    let mut slow_sorted: Vec<usize> = result.slow_indices.clone();
    slow_sorted.sort_unstable_by(|&a, &b| result.durations[b].partial_cmp(&result.durations[a]).unwrap());

    println!("── 卡顿帧详情 (Top 5 最慢帧) ────────────────────────────────");
    for &fi in slow_sorted.iter().take(5) {
        let frame = &data.frames[fi];
        let dur = result.durations[fi];

        println!("\n  ┌─ 帧 #{fi} ({}) {:.1} ms / {} 事件 ──────────────", frame.thread_name, dur, frame.events.len());

        let mut frame_funcs: HashMap<u32, (f64, u32)> = HashMap::new();
        for ev in &frame.events {
            if ev.self_ticks <= 0 { continue; }
            let entry = frame_funcs.entry(ev.desc_idx).or_insert((0.0, 0));
            entry.0 += stats::ticks_to_ms(ev.self_ticks, data.frequency);
            entry.1 += 1;
        }
        let mut top10: Vec<(u32, f64, u32)> = frame_funcs.into_iter()
            .map(|(k, (ms, c))| (k, ms, c))
            .collect();
        top10.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        println!("  │ Top 10 独占时间:");
        println!("  │   独占(ms)   占比    次数  平均/次  函数名 [文件]");
        for (desc_idx, self_ms, count) in top10.iter().take(10) {
            let desc = &data.evt_descs[*desc_idx as usize];
            let p = if dur > 0.0 { self_ms / dur * 100.0 } else { 0.0 };
            let avg = if *count > 0 { self_ms / *count as f64 } else { 0.0 };
            println!("  │   {:>8.1}  {:>5.1}%  {:>5}  {:>7.1}  {} [{}]",
                self_ms, p, count, avg, desc.name, stats::file_short(&desc.file));
        }
    }
    println!();
}

fn print_cross_frame_hotspots(result: &AnalysisResult) {
    println!("── 跨帧热点函数 (卡顿帧中按总独占时间排序) ──────────────────");
    println!("  排名  总独占(ms)  最大(ms)  总次数  平均/次  出现帧数  函数名");
    for (rank, f) in result.all_funcs.iter().take(30).enumerate() {
        let avg = if f.call_count > 0 { f.total_self / f.call_count as f64 } else { 0.0 };
        println!("  {:>4}  {:>10.1}  {:>8.1}  {:>6}  {:>7.1}  {}/{:>3}    {}",
            rank + 1, f.total_self, f.max_self, f.call_count, avg, f.frame_count, result.slow_count, f.name);
    }
    println!();
}

fn print_module_aggregation(result: &AnalysisResult) {
    println!("── 模块级聚合 (按源文件) ─────────────────────────────────────");
    println!("  排名  独占(ms)    占比     函数数  模块");
    for (rank, (file, ms, func_n)) in result.top_modules.iter().take(15).enumerate() {
        let p = if result.total_slow_ms > 0.0 { ms / result.total_slow_ms * 100.0 } else { 0.0 };
        println!("  {:>4}  {:>10.1}  {:>5.1}%  {:>6}  {}", rank + 1, ms, p, func_n, file);
    }
    println!();
}

fn print_call_frequency(result: &AnalysisResult) {
    println!("── 调用频次分析 (按总调用次数排序) ───────────────────────────");
    println!("  排名  总次数   平均独占/次  总独占(ms)  函数名");
    for (rank, &idx) in result.by_call_count_indices.iter().take(10).enumerate() {
        let f = &result.all_funcs[idx];
        let avg = if f.call_count > 0 { f.total_self / f.call_count as f64 } else { 0.0 };
        println!("  {:>4}  {:>6}   {:>10.3}  {:>10.1}  {}", rank + 1, f.call_count, avg, f.total_self, f.name);
    }
    println!();
}

fn print_stability(result: &AnalysisResult) {
    println!("── 函数稳定性分析 (按变异系数降序, 仅出现≥2帧的函数) ────────");
    println!("  函数名                                      平均独占  标准差   变异系数  评级");
    for &(idx, m, s, cv) in result.stability.iter().take(15) {
        let f = &result.all_funcs[idx];
        let rating = if cv < 0.5 { "稳定" } else if cv < 1.0 { "中等" } else { "不稳定" };
        println!("  {:<42}  {:>7.1}  {:>7.1}  {:>7.2}  {}", f.name, m, s, cv, rating);
    }
    println!();
}

fn print_caller_callee(data: &ParsedData, result: &AnalysisResult) {
    println!("── 热点函数调用者分析 (Top 3) ────────────────────────────────");
    for (rank, f) in result.all_funcs.iter().take(3).enumerate() {
        println!("\n  ┌─ #{} {} [{}] ──────────────", rank + 1, f.name, stats::file_short(&f.file));

        if let Some(callers) = result.caller_map.get(&f.desc_idx) {
            let mut sorted_callers: Vec<(u32, u32)> = callers.iter().map(|(&k, &v)| (k, v)).collect();
            sorted_callers.sort_unstable_by(|a, b| b.1.cmp(&a.1));
            println!("  │ 被以下函数调用:");
            for (parent_desc, count) in sorted_callers.iter().take(5) {
                let pdesc = &data.evt_descs[*parent_desc as usize];
                println!("  │   {:>5}×  {} [{}]", count, pdesc.name, stats::file_short(&pdesc.file));
            }
        } else {
            println!("  │ (无调用者 — 顶层事件)");
        }

        if let Some(callees) = result.callee_map.get(&f.desc_idx) {
            let mut sorted_callees: Vec<(u32, u32)> = callees.iter().map(|(&k, &v)| (k, v)).collect();
            sorted_callees.sort_unstable_by(|a, b| b.1.cmp(&a.1));
            println!("  │ 调用了以下子函数 (Top 5):");
            for (child_desc, count) in sorted_callees.iter().take(5) {
                let cdesc = &data.evt_descs[*child_desc as usize];
                println!("  │   {:>5}×  {} [{}]", count, cdesc.name, stats::file_short(&cdesc.file));
            }
        }
    }
    println!();
}

fn print_critical_path(result: &AnalysisResult) {
    if !result.critical_path.is_empty() {
        println!("── 关键路径 (最慢帧中最热函数的调用链) ───────────────────────");
        for (level, (name, file)) in result.critical_path.iter().enumerate() {
            let indent = "  ".repeat(level);
            let arrow = if level < result.critical_path.len() - 1 { "├─" } else { "└─" };
            println!("  {}{}{} [{}]", indent, arrow, name, file);
        }
        println!("  最大调用深度: {}\n", result.max_depth);
    }
}

fn print_suggestions(data: &ParsedData, result: &AnalysisResult) {
    println!("── 自动化优化建议 ────────────────────────────────────────────");

    // 瓶颈函数
    if let Some(top1) = result.all_funcs.first() {
        let pct = if result.total_slow_ms > 0.0 { top1.total_self / result.total_slow_ms * 100.0 } else { 0.0 };
        if pct > 50.0 {
            println!("  [严重] '{}' 独占 {:.1}% 卡顿时间，是首要瓶颈", top1.name, pct);
            println!("         出现在 {}/{} 卡顿帧，单帧最高 {:.0} ms", top1.frame_count, result.slow_count, top1.max_self);
            println!("         建议: 优先优化此函数，考虑异步化/分帧/并行化/减少计算量\n");
        } else if pct > 20.0 {
            println!("  [主要] '{}' 占 {:.1}% 卡顿时间\n", top1.name, pct);
        }
    }

    // 高频调用函数
    for &idx in result.by_call_count_indices.iter().take(3) {
        let f = &result.all_funcs[idx];
        if f.call_count > 1000 {
            let avg = f.total_self / f.call_count as f64;
            println!("  [高频] '{}' 总调用 {} 次，平均 {:.3} ms/次", f.name, f.call_count, avg);
            println!("         总独占 {:.1} ms — 考虑减少调用次数或批处理\n", f.total_self);
        }
    }

    // 不稳定函数
    for &(idx, m, s, cv) in result.stability.iter().take(3) {
        let f = &result.all_funcs[idx];
        if cv > 1.0 && f.per_frame_self.len() >= 3 {
            println!("  [不稳定] '{}' 变异系数 {:.2} (均值 {:.1} ms, 标准差 {:.1} ms)", f.name, cv, m, s);
            println!("           偶发性 spike — 排查触发条件 (数据量/缓存/锁竞争)\n");
        }
    }

    // 多线程卡顿
    let slow_threads: Vec<&str> = result.slow_indices.iter()
        .map(|&i| data.frames[i].thread_name.as_str())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    if slow_threads.len() > 1 {
        println!("  [多线程] {} 个线程出现卡顿帧: {}", slow_threads.len(), slow_threads.join(", "));
        println!("           检查线程间锁竞争或资源争用\n");
    }

    // 调用链过深
    if result.max_depth > 10 {
        println!("  [调用链] 最慢帧最大调用深度 {} 层 — 考虑扁平化逻辑\n", result.max_depth);
    }
}