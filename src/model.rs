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
    pub per_frame_self: Vec<f64>,
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
}