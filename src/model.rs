//! 数据模型: 解析产物 + 分析产物

use std::collections::HashMap;

// ── 解析产物 ──

pub struct ThreadDesc { pub name: String }
pub struct FiberDesc { #[allow(dead_code)] pub id: u64 }
pub struct EventDesc { pub name: String, pub file: String, #[allow(dead_code)] pub line: u32 }
pub struct Event { pub start: i64, pub finish: i64, pub desc_idx: u32, pub self_ticks: i64, pub parent_idx: Option<usize> }
pub struct Frame { pub thread_name: String, pub frame_start: i64, pub frame_finish: i64, pub events: Vec<Event> }
pub struct Block { pub type_: u16, #[allow(dead_code)] pub size: u32, pub payload_start: usize, pub payload_end: usize }

pub struct ParsedData {
    pub threads: Vec<ThreadDesc>,
    #[allow(dead_code)]
    pub fibers: Vec<FiberDesc>,
    pub evt_descs: Vec<EventDesc>,
    pub frames: Vec<Frame>,
    pub frequency: u64,
    pub evt_count: u32,
    pub file_size_mb: f64,
}

// ── 分析产物 ──

pub struct FuncAgg {
    pub desc_idx: u32,
    pub name: String,
    pub file: String,
    pub total_self: f64,
    pub max_self: f64,
    pub call_count: u32,
    pub frame_count: u32,
    /// (frame_index, self_ms) — 仅记录函数有非零 self 的卡顿帧
    pub per_frame_self: Vec<(usize, f64)>,
}

// ── 趋势分析产物 ──

#[derive(Clone, Copy, PartialEq)]
pub enum TrendPattern {
    /// 持续偏高: 出现率高 + 变异系数低
    PersistentHigh,
    /// 偶发尖峰: 出现率低 + 变异系数高
    SporadicSpike,
    /// 递增趋势
    GradualIncrease,
    /// 递减趋势
    GradualDecrease,
    /// 稳定: 出现率高 + 极低变异
    Stable,
}

pub struct FuncTrend {
    #[allow(dead_code)]
    pub desc_idx: u32,
    pub name: String,
    pub file: String,
    /// 逐帧 self_ms 序列 (按 slow_indices 顺序, 0.0 = 该帧未出现)
    pub sparkline: Vec<f64>,
    pub pattern: TrendPattern,
    pub mean_ms: f64,
    pub std_ms: f64,
    /// 变异系数 CV = std/mean
    pub cv: f64,
    pub max_ms: f64,
    /// 出现率 = 有非零值的帧数 / 总卡顿帧数
    pub appearance_rate: f64,
    /// 简单线性回归斜率 (ms/帧)
    pub trend_slope: f64,
}

pub struct FuncCorrelation {
    pub name_a: String,
    pub name_b: String,
    /// Pearson 相关系数 [-1.0, 1.0]
    pub pearson: f64,
    /// 同时出现高耗时的帧数
    pub co_occurrence: usize,
}

pub struct TrendAnalysis {
    pub func_trends: Vec<FuncTrend>,
    pub correlations: Vec<FuncCorrelation>,
}

pub struct AnalysisResult {
    // 帧统计
    pub frame_count: usize,
    pub total_events: usize,
    pub durations: Vec<f64>,
    pub sorted_dur: Vec<f64>,
    pub min_ms: f64,
    pub max_ms: f64,
    pub mean_ms: f64,
    pub std_ms: f64,

    // 卡顿帧
    pub threshold: f64,
    pub slow_indices: Vec<usize>,
    pub slow_count: usize,
    pub total_slow_ms: f64,
    pub over_60fps: usize,
    pub over_30fps: usize,

    // 线程分析
    pub thread_stats: Vec<(String, u32, f64, usize)>,

    // 函数热点
    pub all_funcs: Vec<FuncAgg>,
    pub top_modules: Vec<(String, f64, usize)>,
    pub by_call_count_indices: Vec<usize>,
    pub stability: Vec<(usize, f64, f64, f64)>,

    // 调用关系
    pub caller_map: HashMap<u32, HashMap<u32, u32>>,
    pub callee_map: HashMap<u32, HashMap<u32, u32>>,

    // Pareto
    pub top1_self: f64,
    pub top3_self: f64,
    pub top5_self: f64,
    pub top10_self: f64,

    // 关键路径
    pub critical_path: Vec<(String, String)>,
    pub max_depth: usize,

    // 趋势分析
    pub trend: TrendAnalysis,
}