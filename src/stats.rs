//! 统计辅助函数

pub fn ticks_to_ms(t: i64, freq: u64) -> f64 {
    t as f64 * 1000.0 / freq as f64
}

pub fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() { return 0.0; }
    let idx = (sorted.len() as f64 * p) as usize;
    sorted[idx.min(sorted.len() - 1)]
}

pub fn mean_std(v: &[f64]) -> (f64, f64) {
    if v.is_empty() { return (0.0, 0.0); }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64;
    (m, var.sqrt())
}

pub fn bar(ms: f64) -> String {
    let n = (ms / 1000.0).round() as usize;
    "█".repeat(n.min(40))
}

pub fn file_short(f: &str) -> String {
    f.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(f).to_string()
}