//! 趋势分析: 模式分类 + 交叉函数相关性
//!
//! 数据流:
//!   all_funcs (按 total_self 降序) + slow_indices
//!     → 为 Top N 函数构建逐帧 sparkline 序列
//!     → 分类模式 (PersistentHigh / SporadicSpike / GradualIncrease / GradualDecrease / Stable)
//!     → 计算 Top N 之间的 Pearson 相关性 → 发现共退化函数对

use crate::model::*;
use crate::stats;

/// 用于趋势分析的 Top N 函数数量
const TREND_TOP_N: usize = 15;
/// Pearson 绝对值阈值 — 超过此值才报告相关性
const CORR_THRESHOLD: f64 = 0.7;

pub fn analyze_trends(
    all_funcs: &[FuncAgg],
    slow_indices: &[usize],
    _data: &ParsedData,
) -> TrendAnalysis {
    let slow_count = slow_indices.len();
    if slow_count == 0 {
        return TrendAnalysis {
            func_trends: Vec::new(),
            correlations: Vec::new(),
        };
    }

    // slow_indices → 帧索引查找表 (frame_index_in_slow → original_frame_index)
    let slow_set: std::collections::HashSet<usize> = slow_indices.iter().copied().collect();

    // ── 为 Top N 函数构建逐帧 sparkline ──
    // sparkline[i] = 函数在 slow_indices[i] 帧的 self_ms (0.0 = 未出现)
    let top_n = all_funcs.iter().take(TREND_TOP_N).collect::<Vec<_>>();
    let mut sparklines: Vec<Vec<f64>> = Vec::with_capacity(top_n.len());

    for func in &top_n {
        let mut series = vec![0.0; slow_count];
        // per_frame_self 已按帧序追加，直接填充
        for &(fi, ms) in &func.per_frame_self {
            // fi 是原始帧索引，需找到在 slow_indices 中的位置
            if slow_set.contains(&fi) {
                if let Some(pos) = slow_indices.iter().position(|&x| x == fi) {
                    series[pos] = ms;
                }
            }
        }
        sparklines.push(series);
    }

    // ── 模式分类 ──
    let func_trends: Vec<FuncTrend> = top_n
        .iter()
        .enumerate()
        .map(|(i, func)| {
            let series = &sparklines[i];
            let non_zero: Vec<f64> = series.iter().filter(|&&v| v > 0.0).copied().collect();
            let (mean_ms, std_ms) = stats::mean_std(&non_zero);
            let cv = if mean_ms > 0.0 { std_ms / mean_ms } else { 0.0 };
            let max_ms = series.iter().cloned().fold(0.0f64, f64::max);
            let appearance_rate = non_zero.len() as f64 / slow_count as f64;
            let (slope, _) = stats::linreg(series);

            let pattern = classify(
                appearance_rate,
                cv,
                slope,
                mean_ms,
            );

            FuncTrend {
                desc_idx: func.desc_idx,
                name: func.name.clone(),
                file: stats::file_short(&func.file),
                sparkline: series.clone(),
                pattern,
                mean_ms,
                std_ms,
                cv,
                max_ms,
                appearance_rate,
                trend_slope: slope,
            }
        })
        .collect();

    // ── 交叉函数相关性 (Pearson) ──
    let mut correlations: Vec<FuncCorrelation> = Vec::new();
    for i in 0..top_n.len() {
        for j in (i + 1)..top_n.len() {
            let r = stats::pearson(&sparklines[i], &sparklines[j]);
            if r.abs() >= CORR_THRESHOLD {
                // 同时出现高耗时 (>均值) 的帧数
                let mean_a = func_trends[i].mean_ms;
                let mean_b = func_trends[j].mean_ms;
                let co_occur = sparklines[i]
                    .iter()
                    .zip(sparklines[j].iter())
                    .filter(|(&a, &b)| a > mean_a && b > mean_b)
                    .count();
                correlations.push(FuncCorrelation {
                    name_a: func_trends[i].name.clone(),
                    name_b: func_trends[j].name.clone(),
                    pearson: r,
                    co_occurrence: co_occur,
                });
            }
        }
    }
    // 按绝对相关系数降序
    correlations.sort_unstable_by(|a, b| b.pearson.abs().partial_cmp(&a.pearson.abs()).unwrap());

    TrendAnalysis {
        func_trends,
        correlations,
    }
}

/// 模式分类逻辑
///
/// - appearance_rate: 出现率 [0, 1]
/// - cv: 变异系数 (std/mean)
/// - slope: 线性回归斜率 (ms/帧)
/// - mean_ms: 平均值
fn classify(appearance_rate: f64, cv: f64, slope: f64, mean_ms: f64) -> TrendPattern {
    // 显著趋势: 斜率 > 均值的 10% 且方向明确
    let slope_ratio = if mean_ms > 0.0 { slope / mean_ms } else { 0.0 };
    if slope_ratio > 0.1 && appearance_rate > 0.5 {
        return TrendPattern::GradualIncrease;
    }
    if slope_ratio < -0.1 && appearance_rate > 0.5 {
        return TrendPattern::GradualDecrease;
    }

    // 出现率 + 变异系数分类
    if appearance_rate >= 0.7 && cv < 0.3 {
        TrendPattern::Stable
    } else if appearance_rate >= 0.6 && cv < 0.8 {
        TrendPattern::PersistentHigh
    } else if appearance_rate < 0.4 && cv > 1.0 {
        TrendPattern::SporadicSpike
    } else if appearance_rate >= 0.5 && cv < 0.5 {
        TrendPattern::Stable
    } else {
        TrendPattern::SporadicSpike
    }
}