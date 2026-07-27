//! 分析聚合: 解析数据 + 阈值 → 分析结果

use std::collections::HashMap;
use crate::model::*;
use crate::stats;

pub fn analyze(data: &ParsedData, threshold: f64) -> AnalysisResult {
    let freq = data.frequency;
    let to_ms = |t: i64| stats::ticks_to_ms(t, freq);

    // 预计算 file_short (每个 EventDesc 只算一次，热循环中直接索引)
    let file_shorts: Vec<String> = data.evt_descs.iter()
        .map(|d| stats::file_short(&d.file))
        .collect();

    // ═══ 帧统计 ═══
    let frame_count = data.frames.len();
    let total_events: usize = data.frames.iter().map(|f| f.events.len()).sum();

    let durations: Vec<f64> = data.frames.iter()
        .map(|f| to_ms(f.frame_finish - f.frame_start))
        .collect();
    let mut sorted_dur = durations.clone();
    sorted_dur.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

    let min_ms = sorted_dur.first().copied().unwrap_or(0.0);
    let max_ms = sorted_dur.last().copied().unwrap_or(0.0);
    let (mean_ms, std_ms) = stats::mean_std(&sorted_dur);

    // ═══ 卡顿帧判定 ═══
    let slow_indices: Vec<usize> = (0..frame_count)
        .filter(|&i| durations[i] >= threshold)
        .collect();
    let slow_count = slow_indices.len();
    let total_slow_ms: f64 = slow_indices.iter().map(|&i| durations[i]).sum();

    // ═══ 帧预算分析 ═══
    let over_60fps = sorted_dur.iter().filter(|&&d| d > 16.67).count();
    let over_30fps = sorted_dur.iter().filter(|&&d| d > 33.33).count();

    // ═══ 线程分析 ═══
    let mut thread_map: HashMap<String, (u32, f64, usize)> = HashMap::with_capacity(data.threads.len());
    for (i, f) in data.frames.iter().enumerate() {
        let e = thread_map.entry(f.thread_name.clone()).or_insert((0, 0.0, 0));
        e.0 += 1;
        e.1 += durations[i];
        e.2 += f.events.len();
    }
    let mut thread_stats: Vec<(String, u32, f64, usize)> = thread_map.into_iter()
        .map(|(name, (count, total_ms, events))| (name, count, total_ms, events))
        .collect();
    thread_stats.sort_unstable_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    // ═══ 跨帧热点聚合 (仅卡顿帧) ═══
    let mut func_map: HashMap<u32, FuncAgg> = HashMap::with_capacity(data.evt_descs.len());
    let mut module_map: HashMap<String, (f64, HashMap<String, ()>)> = HashMap::new();
    let mut caller_map: HashMap<u32, HashMap<u32, u32>> = HashMap::new();
    let mut callee_map: HashMap<u32, HashMap<u32, u32>> = HashMap::new();

    for &fi in &slow_indices {
        let frame = &data.frames[fi];
        let mut seen_in_frame: HashMap<u32, f64> = HashMap::with_capacity(256);

        for ev in &frame.events {
            if ev.self_ticks <= 0 { continue; }
            let desc_idx = ev.desc_idx as usize;
            let desc = &data.evt_descs[desc_idx];
            let self_ms = to_ms(ev.self_ticks);

            // 函数级聚合
            let agg = func_map.entry(ev.desc_idx).or_insert_with(|| FuncAgg {
                desc_idx: ev.desc_idx,
                name: desc.name.clone(),
                file: desc.file.clone(),
                total_self: 0.0,
                max_self: 0.0,
                call_count: 0,
                frame_count: 0,
                per_frame_self: Vec::new(),
            });
            agg.total_self += self_ms;
            agg.max_self = agg.max_self.max(self_ms);
            agg.call_count += 1;

            let seen = seen_in_frame.entry(ev.desc_idx).or_insert(0.0);
            *seen += self_ms;

            // 模块级聚合 — 用预计算的 file_short，避免热循环中重复 rsplit
            // contains_key + get_mut 双查询，但避免了 N 次 String clone
            let fname = &file_shorts[desc_idx];
            let ment = if module_map.contains_key(fname) {
                module_map.get_mut(fname).unwrap()
            } else {
                module_map.entry(fname.clone()).or_insert_with(|| (0.0, HashMap::new()))
            };
            ment.0 += self_ms;
            // 只在首次出现时 clone，避免 N 次冗余分配
            if !ment.1.contains_key(&desc.name) {
                ment.1.insert(desc.name.clone(), ());
            }

            // 调用者/被调用者关系
            if let Some(pidx) = ev.parent_idx {
                let parent_desc = frame.events[pidx].desc_idx;
                *caller_map.entry(ev.desc_idx).or_insert_with(HashMap::new)
                    .entry(parent_desc).or_insert(0) += 1;
                *callee_map.entry(parent_desc).or_insert_with(HashMap::new)
                    .entry(ev.desc_idx).or_insert(0) += 1;
            }
        }

        // 记录每帧的 self time (用于稳定性分析)
        for (desc_idx, frame_self) in &seen_in_frame {
            if let Some(agg) = func_map.get_mut(desc_idx) {
                agg.per_frame_self.push(*frame_self);
                agg.frame_count += 1;
            }
        }
    }

    // 排序函数热点 (按 total_self 降序)
    let mut all_funcs: Vec<FuncAgg> = func_map.into_values().collect();
    all_funcs.sort_unstable_by(|a, b| b.total_self.partial_cmp(&a.total_self).unwrap());

    // 排序模块热点
    let mut top_modules: Vec<(String, f64, usize)> = module_map.into_iter()
        .map(|(file, (ms, funcs))| (file, ms, funcs.len()))
        .collect();
    top_modules.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // 调用频次排序 (索引 into all_funcs)
    let mut by_call_count_indices: Vec<usize> = (0..all_funcs.len())
        .filter(|&i| all_funcs[i].call_count > 0)
        .collect();
    by_call_count_indices.sort_unstable_by(|&a, &b| all_funcs[b].call_count.cmp(&all_funcs[a].call_count));

    // 稳定性分析 (CV = std/mean)
    let mut stability: Vec<(usize, f64, f64, f64)> = (0..all_funcs.len())
        .filter(|&i| all_funcs[i].per_frame_self.len() >= 2)
        .map(|i| {
            let (m, s) = stats::mean_std(&all_funcs[i].per_frame_self);
            let cv = if m > 0.0 { s / m } else { 0.0 };
            (i, m, s, cv)
        })
        .collect();
    stability.sort_unstable_by(|a, b| b.3.partial_cmp(&a.3).unwrap());

    // Pareto 分析
    let top1_self = all_funcs.first().map(|f| f.total_self).unwrap_or(0.0);
    let top3_self: f64 = all_funcs.iter().take(3).map(|f| f.total_self).sum();
    let top5_self: f64 = all_funcs.iter().take(5).map(|f| f.total_self).sum();
    let top10_self: f64 = all_funcs.iter().take(10).map(|f| f.total_self).sum();

    // 关键路径 (最慢帧中最热函数的调用链)
    let slowest_idx = slow_indices.iter()
        .max_by(|&a, &b| durations[*a].partial_cmp(&durations[*b]).unwrap())
        .copied();
    let mut critical_path: Vec<(String, String)> = Vec::new();
    let mut max_depth = 0usize;
    if let Some(si) = slowest_idx {
        let frame = &data.frames[si];
        let hottest = frame.events.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.self_ticks.cmp(&b.self_ticks))
            .map(|(i, _)| i);
        if let Some(hi) = hottest {
            let mut cur = hi;
            let mut chain = Vec::new();
            while let Some(ev) = frame.events.get(cur) {
                let desc_idx = ev.desc_idx as usize;
                let desc = &data.evt_descs[desc_idx];
                chain.push((desc.name.clone(), file_shorts[desc_idx].clone()));
                cur = match ev.parent_idx { Some(p) => p, None => break };
            }
            critical_path = chain.into_iter().rev().collect();
        }
        let mut max_d = 0usize;
        let mut stack: Vec<usize> = Vec::new();
        for i in 0..frame.events.len() {
            while let Some(&t) = stack.last() {
                if frame.events[t].finish <= frame.events[i].start { stack.pop(); } else { break; }
            }
            stack.push(i);
            max_d = max_d.max(stack.len());
        }
        max_depth = max_d;
    }

    AnalysisResult {
        frame_count, total_events, durations, sorted_dur,
        min_ms, max_ms, mean_ms, std_ms,
        threshold, slow_indices, slow_count, total_slow_ms,
        over_60fps, over_30fps,
        thread_stats,
        all_funcs, top_modules, by_call_count_indices, stability,
        caller_map, callee_map,
        top1_self, top3_self, top5_self, top10_self,
        critical_path, max_depth,
    }
}