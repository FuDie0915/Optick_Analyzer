//! 统计辅助函数

const FRAC_BLOCKS: [&str; 9] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

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

/// 精细进度条: val 占 max_val 比例映射到 width 格，用 1/8 分数块字符
pub fn bar_scaled(val: f64, max_val: f64, width: usize) -> String {
    if max_val <= 0.0 || val <= 0.0 { return " ".repeat(width); }
    let scaled = (val / max_val).min(1.0) * width as f64;
    let whole = scaled as usize;
    let frac = scaled - whole as f64;
    let mut s = "█".repeat(whole.min(width));
    if whole < width {
        let idx = (frac * 8.0).round() as usize;
        s.push_str(FRAC_BLOCKS[idx.min(8)]);
    }
    while s.chars().count() < width { s.push(' '); }
    s
}

/// 帧状态符号
#[allow(dead_code)]
pub fn frame_status(dur: f64, threshold: f64) -> &'static str {
    if dur >= threshold { "▲" }
    else if dur > 33.33 { "◆" }
    else if dur > 16.67 { "◇" }
    else { "●" }
}

pub fn file_short(f: &str) -> String {
    f.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(f).to_string()
}

/// Pearson 相关系数: 衡量两条序列的线性相关性
/// 返回 [-1.0, 1.0]，NaN 当任一序列方差为 0
pub fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len()) as f64;
    if n < 2.0 { return 0.0; }
    let ma = a.iter().sum::<f64>() / n;
    let mb = b.iter().sum::<f64>() / n;
    let mut cov = 0.0;
    let mut va = 0.0;
    let mut vb = 0.0;
    for i in 0..a.len().min(b.len()) {
        let da = a[i] - ma;
        let db = b[i] - mb;
        cov += da * db;
        va += da * da;
        vb += db * db;
    }
    let denom = (va * vb).sqrt();
    if denom == 0.0 { 0.0 } else { cov / denom }
}

/// 简单线性回归斜率 (最小二乘法)
/// 返回 (slope, intercept) — slope = 每帧变化量
pub fn linreg(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    if n < 2.0 { return (0.0, 0.0); }
    let xs: Vec<f64> = (0..values.len()).map(|i| i as f64).collect();
    let mx = xs.iter().sum::<f64>() / n;
    let my = values.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for i in 0..values.len() {
        num += (xs[i] - mx) * (values[i] - my);
        den += (xs[i] - mx).powi(2);
    }
    let slope = if den == 0.0 { 0.0 } else { num / den };
    let intercept = my - slope * mx;
    (slope, intercept)
}