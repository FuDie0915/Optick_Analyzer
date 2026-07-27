//! HTML 报告生成 — 自包含单文件, 内联 CSS/JS/SVG
//!
//! 数据流:
//!   ParsedData + AnalysisResult + input_path
//!     → 各 section_* 函数生成 HTML 片段
//!     → 拼装为完整 HTML 文档字符串
//!
//! 设计原则:
//!   - 零外部依赖 (无 CDN, 无外部 CSS/JS)
//!   - SVG 图表内联生成 (sparkline / bar chart / pareto)
//!   - 暗色主题, 响应式布局
//!   - 可折叠详情区域 (vanilla JS)

use crate::model::*;
use crate::stats;
use std::fmt::Write as _;

// ── HTML 转义 ──

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

// ── SVG 生成 ──

/// 生成 SVG sparkline 折线图
fn svg_sparkline(values: &[f64], color: &str, w: usize, h: usize) -> String {
    if values.is_empty() { return format!(r#"<svg width="{w}" height="{h}"></svg>"#); }
    let max = values.iter().cloned().fold(0.0f64, f64::max).max(1e-9);
    let n = values.len() as f64;
    let step = w as f64 / n.max(1.0);
    let mut points = String::new();
    for (i, &v) in values.iter().enumerate() {
        let x = i as f64 * step;
        let y = h as f64 - (v / max * (h as f64 - 2.0)) - 1.0;
        let _ = write!(points, "{:.1},{:.1} ", x, y);
    }
    // 面积填充
    let area_pts = format!("0,{} {} {}", h, points.trim(), format!("{:.1},{}", w as f64, h));
    format!(
        r#"<svg class="spark" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><polygon points="{area_pts}" fill="{color}" opacity="0.15"/><polyline points="{points}" fill="none" stroke="{color}" stroke-width="1.5"/></svg>"#
    )
}

/// 生成 SVG 垂直柱状图 (帧时间线)
fn svg_frame_chart(durations: &[f64], threshold: f64) -> String {
    let n = durations.len();
    if n == 0 { return String::new(); }
    let max_dur = durations.iter().cloned().fold(0.0f64, f64::max).max(1e-9);
    let chart_w = 1000;
    let chart_h = 200;
    let bar_w = (chart_w / n).max(2);
    let gap = if bar_w > 4 { 1 } else { 0 };
    let mut bars = String::new();
    for (i, &d) in durations.iter().enumerate() {
        let x = i * bar_w;
        let bar_h = (d / max_dur * (chart_h as f64 - 20.0)) as usize;
        let y = chart_h - bar_h - 15;
        let color = if d >= threshold { "#e94560" }
            else if d > 33.33 { "#f39c12" }
            else if d > 16.67 { "#f1c40f" }
            else { "#2ecc71" };
        let _ = write!(bars,
            r#"<rect x="{x}" y="{y}" width="{}" height="{bar_h}" fill="{color}" rx="1"><title>帧 {i}: {d:.1} ms</title></rect>"#,
            bar_w - gap);
    }
    // 阈值参考线
    let thresh_y = chart_h - ((threshold / max_dur * (chart_h as f64 - 20.0)) as usize) - 15;
    format!(
        r##"<svg class="frame-chart" viewBox="0 0 {chart_w} {chart_h}" preserveAspectRatio="none">{bars}<line x1="0" y1="{thresh_y}" x2="{chart_w}" y2="{thresh_y}" stroke="#e94560" stroke-width="1" stroke-dasharray="4,2" opacity="0.6"/></svg>"##
    )
}

// ── 模式标签 ──

fn pattern_badge(p: &TrendPattern) -> &'static str {
    match p {
        TrendPattern::PersistentHigh => r#"<span class="badge" style="background:#c0392b">持续偏高</span>"#,
        TrendPattern::SporadicSpike => r#"<span class="badge" style="background:#e67e22">偶发尖峰</span>"#,
        TrendPattern::GradualIncrease => r#"<span class="badge" style="background:#e74c3c">递增</span>"#,
        TrendPattern::GradualDecrease => r#"<span class="badge" style="background:#27ae60">递减</span>"#,
        TrendPattern::Stable => r#"<span class="badge" style="background:#2ecc71">稳定</span>"#,
    }
}

fn pattern_color(p: &TrendPattern) -> &'static str {
    match p {
        TrendPattern::PersistentHigh => "#e74c3c",
        TrendPattern::SporadicSpike => "#e67e22",
        TrendPattern::GradualIncrease => "#c0392b",
        TrendPattern::GradualDecrease => "#27ae60",
        TrendPattern::Stable => "#2ecc71",
    }
}

// ── 进度条 (CSS) ──

fn css_bar(pct: f64, color: &str) -> String {
    let w = pct.clamp(0.0, 100.0);
    format!(r#"<div class="bar-track"><div class="bar-fill" style="width:{w:.1}%;background:{color}"></div></div>"#)
}

// ── CSS / JS ──

fn css() -> &'static str {
    r#"
:root {
  --bg: #0f0f1e; --card: #1a1a2e; --card2: #16213e; --border: #2a2a4a;
  --text: #e0e0e0; --text2: #a0a0b0; --accent: #0f3460; --link: #4db8ff;
  --red: #e94560; --orange: #f39c12; --yellow: #f1c40f; --green: #2ecc71;
  --blue: #3498db; --purple: #9b59b6;
}
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
  font-family: -apple-system, 'Segoe UI', Roboto, sans-serif;
  background: var(--bg); color: var(--text); line-height: 1.6; padding: 20px;
}
.container { max-width: 1200px; margin: 0 auto; }
header { text-align: center; padding: 30px 0 20px; border-bottom: 1px solid var(--border); margin-bottom: 30px; }
header h1 { font-size: 28px; color: var(--text); margin-bottom: 8px; }
header .meta { color: var(--text2); font-size: 14px; }
section { background: var(--card); border: 1px solid var(--border); border-radius: 8px; padding: 20px; margin-bottom: 20px; }
section h2 { font-size: 18px; color: var(--link); margin-bottom: 16px; padding-bottom: 8px; border-bottom: 1px solid var(--border); }
.cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 12px; margin-bottom: 8px; }
.card { background: var(--card2); border-radius: 6px; padding: 16px; text-align: center; }
.card .label { color: var(--text2); font-size: 12px; text-transform: uppercase; letter-spacing: 0.5px; }
.card .value { font-size: 28px; font-weight: 600; margin-top: 4px; }
.card .value.red { color: var(--red); }
.card .value.orange { color: var(--orange); }
.card .value.green { color: var(--green); }
table { width: 100%; border-collapse: collapse; font-size: 13px; }
th, td { padding: 8px 10px; text-align: left; border-bottom: 1px solid var(--border); }
th { color: var(--text2); font-weight: 600; font-size: 12px; text-transform: uppercase; letter-spacing: 0.3px; }
tr:hover { background: var(--card2); }
td.num { text-align: right; font-variant-numeric: tabular-nums; }
.bar-track { background: var(--bg); border-radius: 3px; height: 8px; overflow: hidden; min-width: 60px; }
.bar-fill { height: 100%; border-radius: 3px; transition: width 0.3s; }
.spark { vertical-align: middle; }
.frame-chart { width: 100%; height: 200px; }
.badge { display: inline-block; padding: 2px 8px; border-radius: 10px; font-size: 11px; font-weight: 600; color: #fff; }
.collapsible { cursor: pointer; user-select: none; }
.collapsible::before { content: '▼ '; font-size: 10px; color: var(--text2); }
.collapsible.collapsed::before { content: '▶ '; }
.collapsed + .collapsible-content { display: none; }
.collapsible-content { transition: display 0.2s; }
.alert { padding: 12px 16px; border-radius: 6px; margin-bottom: 8px; font-size: 13px; }
.alert.severe { background: rgba(233,69,96,0.15); border-left: 3px solid var(--red); }
.alert.major { background: rgba(243,156,18,0.15); border-left: 3px solid var(--orange); }
.alert.freq { background: rgba(52,152,219,0.15); border-left: 3px solid var(--blue); }
.alert.unstable { background: rgba(155,89,182,0.15); border-left: 3px solid var(--purple); }
.alert.thread { background: rgba(241,196,15,0.15); border-left: 3px solid var(--yellow); }
.alert.depth { background: rgba(46,204,113,0.15); border-left: 3px solid var(--green); }
.call-path { font-family: 'Cascadia Code', 'Consolas', monospace; font-size: 13px; }
.call-path .level { display: inline-block; color: var(--text2); }
.correlation-pair { display: flex; align-items: center; gap: 8px; padding: 6px 0; border-bottom: 1px solid var(--border); }
.correlation-pair .names { flex: 1; font-size: 13px; }
.correlation-pair .coeff { font-weight: 600; font-size: 14px; }
.positive { color: var(--red); }
.negative { color: var(--green); }
@media (max-width: 768px) { .cards { grid-template-columns: 1fr 1fr; } table { font-size: 12px; } }
"#
}

fn js() -> &'static str {
    r#"
document.querySelectorAll('.collapsible').forEach(el => {
  el.addEventListener('click', () => {
    el.classList.toggle('collapsed');
    const content = el.nextElementSibling;
    if (content) content.style.display = el.classList.contains('collapsed') ? 'none' : '';
  });
});
document.querySelectorAll('th[data-sort]').forEach(th => {
  th.style.cursor = 'pointer';
  th.addEventListener('click', () => {
    const table = th.closest('table');
    const tbody = table.querySelector('tbody');
    const idx = Array.from(th.parentElement.children).indexOf(th);
    const asc = th.dataset.order === 'asc';
    th.dataset.order = asc ? 'desc' : 'asc';
    const rows = Array.from(tbody.querySelectorAll('tr'));
    rows.sort((a, b) => {
      const va = a.children[idx].dataset.sort || a.children[idx].textContent;
      const vb = b.children[idx].dataset.sort || b.children[idx].textContent;
      const na = parseFloat(va), nb = parseFloat(vb);
      if (!isNaN(na) && !isNaN(nb)) return asc ? na - nb : nb - na;
      return asc ? va.localeCompare(vb) : vb.localeCompare(va);
    });
    rows.forEach(r => tbody.appendChild(r));
  });
});
"#
}

// ── 各报告段落 ──

fn section_summary(data: &ParsedData, result: &AnalysisResult, _input: &str) -> String {
    let mut html = String::new();
    let _ = writeln!(html, r#"<div class="cards">"#);
    let _ = writeln!(html, r#"<div class="card"><div class="label">总帧数</div><div class="value">{}</div></div>"#, result.frame_count);
    let _ = writeln!(html, r#"<div class="card"><div class="label">卡顿帧</div><div class="value red">{}</div></div>"#, result.slow_count);
    let _ = writeln!(html, r#"<div class="card"><div class="label">总事件</div><div class="value">{}</div></div>"#, result.total_events);
    let _ = writeln!(html, r#"<div class="card"><div class="label">均值耗时</div><div class="value">{:.1} ms</div></div>"#, result.mean_ms);
    let _ = writeln!(html, r#"<div class="card"><div class="label">最大耗时</div><div class="value red">{:.1} ms</div></div>"#, result.max_ms);
    let _ = writeln!(html, r#"<div class="card"><div class="label">线程数</div><div class="value">{}</div></div>"#, data.threads.len());
    let _ = writeln!(html, r#"<div class="card"><div class="label">事件描述</div><div class="value">{}</div></div>"#, data.evt_count);
    let _ = writeln!(html, r#"</div>"#);

    // 核心结论
    if let Some(top1) = result.all_funcs.first() {
        let pct = if result.total_slow_ms > 0.0 { top1.total_self / result.total_slow_ms * 100.0 } else { 0.0 };
        let _ = writeln!(html, r#"<div style="margin-top:16px;padding:16px;background:var(--card2);border-radius:6px;border-left:3px solid var(--red)">"#);
        let _ = writeln!(html, r#"<strong>瓶颈函数</strong>: {} ({})<br>"#, esc(&top1.name), esc(&stats::file_short(&top1.file)));
        let _ = writeln!(html, r#"独占 {:.1}% 卡顿时间 ({:.0} ms / {:.0} ms)<br>"#, pct, top1.total_self, result.total_slow_ms);
        let _ = writeln!(html, r#"出现在 {}/{} 卡顿帧，单帧最高 {:.0} ms"#, top1.frame_count, result.slow_count, top1.max_self);
        let _ = writeln!(html, r#"</div>"#);
    }
    html
}

fn section_frame_stats(result: &AnalysisResult) -> String {
    let mut h = String::new();
    h.push_str(r#"<table><thead><tr><th>最快</th><th>P25</th><th>P50</th><th>P75</th><th>P90</th><th>P95</th><th>P99</th><th>最慢</th><th>均值</th><th>标准差</th></tr></thead><tbody><tr>"#);
    let vals = [
        result.min_ms,
        stats::pct(&result.sorted_dur, 0.25),
        stats::pct(&result.sorted_dur, 0.50),
        stats::pct(&result.sorted_dur, 0.75),
        stats::pct(&result.sorted_dur, 0.90),
        stats::pct(&result.sorted_dur, 0.95),
        stats::pct(&result.sorted_dur, 0.99),
        result.max_ms,
        result.mean_ms,
        result.std_ms,
    ];
    for v in &vals {
        let _ = write!(h, r#"<td class="num">{:.1}</td>"#, v);
    }
    h.push_str("</tr></tbody></table>");
    h.push_str(&svg_frame_chart(&result.durations, result.threshold));
    h
}

fn section_frame_budget(result: &AnalysisResult) -> String {
    let total = result.frame_count as f64;
    let p60 = result.over_60fps as f64 / total * 100.0;
    let p30 = result.over_30fps as f64 / total * 100.0;
    let ps = result.slow_count as f64 / total * 100.0;
    format!(
        r#"<table><thead><tr><th>预算</th><th class="num">超标/总数</th><th class="num">超标率</th><th>分布</th></tr></thead><tbody>
        <tr><td>60fps (16.7ms)</td><td class="num">{} / {}</td><td class="num">{:.1}%</td><td>{}</td></tr>
        <tr><td>30fps (33.3ms)</td><td class="num">{} / {}</td><td class="num">{:.1}%</td><td>{}</td></tr>
        <tr><td>自定义 ({:.0}ms)</td><td class="num">{} / {}</td><td class="num">{:.1}%</td><td>{}</td></tr>
        </tbody></table>"#,
        result.over_60fps, result.frame_count, p60, css_bar(p60, "#f1c40f"),
        result.over_30fps, result.frame_count, p30, css_bar(p30, "#f39c12"),
        result.threshold, result.slow_count, result.frame_count, ps, css_bar(ps, "#e94560"),
    )
}

fn section_pareto(result: &AnalysisResult) -> String {
    if result.total_slow_ms <= 0.0 { return String::new(); }
    let mut h = String::new();
    h.push_str(r#"<table><thead><tr><th>范围</th><th class="num">独占 ms</th><th class="num">占比</th><th>分布</th></tr></thead><tbody>"#);
    let items = [("Top 1", result.top1_self), ("Top 3", result.top3_self), ("Top 5", result.top5_self), ("Top 10", result.top10_self)];
    for (label, ms) in &items {
        let pct = ms / result.total_slow_ms * 100.0;
        let _ = write!(h, r#"<tr><td>{}</td><td class="num">{:.1}</td><td class="num">{:.1}%</td><td>{}</td></tr>"#,
            label, ms, pct, css_bar(pct, "#9b59b6"));
    }
    h.push_str("</tbody></table>");
    h
}

fn section_threads(data: &ParsedData, result: &AnalysisResult) -> String {
    let mut h = String::new();
    h.push_str(r#"<table><thead><tr><th>线程名</th><th class="num">帧数</th><th class="num">总耗时 ms</th><th class="num">事件数</th><th class="num">占比</th><th>状态</th></tr></thead><tbody>"#);
    for (name, count, total_ms, events) in &result.thread_stats {
        let pct = if result.total_slow_ms > 0.0 { total_ms / result.total_slow_ms * 100.0 } else { 0.0 };
        let is_slow = result.slow_indices.iter().any(|&i| data.frames[i].thread_name == *name);
        let status = if is_slow { r#"<span class="badge" style="background:#e94560">卡顿</span>"# } else { r#"<span class="badge" style="background:#2ecc71">正常</span>"# };
        let _ = write!(h, r#"<tr><td>{}</td><td class="num">{}</td><td class="num">{:.1}</td><td class="num">{}</td><td class="num">{:.1}%</td><td>{}</td></tr>"#,
            esc(name), count, total_ms, events, pct, status);
    }
    h.push_str("</tbody></table>");
    h
}

fn section_timeline(data: &ParsedData, result: &AnalysisResult) -> String {
    let mut h = String::new();
    h.push_str(r#"<table><thead><tr><th class="num">#</th><th>线程</th><th class="num">耗时 ms</th><th class="num">Δ前帧</th><th class="num">事件</th><th>进度条</th><th>状态</th></tr></thead><tbody>"#);
    let max_dur = result.max_ms.max(1.0);
    let mut prev_ms = 0.0f64;
    for (i, f) in data.frames.iter().enumerate() {
        let dur = result.durations[i];
        let delta = if i > 0 { dur - prev_ms } else { 0.0 };
        prev_ms = dur;
        let (status, color) = if dur >= result.threshold { ("卡顿", "#e94560") }
            else if dur > 33.33 { ("30fps", "#f39c12") }
            else if dur > 16.67 { ("60fps", "#f1c40f") }
            else { ("流畅", "#2ecc71") };
        let delta_str = if i > 0 { format!("{:+.1}", delta) } else { "—".into() };
        let _ = write!(h, r#"<tr><td class="num">{}</td><td>{}</td><td class="num">{:.1}</td><td class="num">{}</td><td class="num">{}</td><td>{}</td><td><span class="badge" style="background:{}">{}</span></td></tr>"#,
            i, esc(&f.thread_name), dur, delta_str, f.events.len(), css_bar(dur / max_dur * 100.0, color), color, status);
    }
    h.push_str("</tbody></table>");
    h
}

fn section_slow_frames(data: &ParsedData, result: &AnalysisResult) -> String {
    let mut slow_sorted: Vec<usize> = result.slow_indices.clone();
    slow_sorted.sort_unstable_by(|&a, &b| result.durations[b].partial_cmp(&result.durations[a]).unwrap());

    let mut h = String::new();
    for &fi in slow_sorted.iter().take(5) {
        let frame = &data.frames[fi];
        let dur = result.durations[fi];
        let _ = write!(h, r#"<div class="collapsible collapsed" style="padding:10px;background:var(--card2);border-radius:6px;margin-bottom:8px;font-weight:600">帧 #{fi} ({}) — {:.1} ms / {} 事件</div>"#,
            esc(&frame.thread_name), dur, frame.events.len());
        h.push_str(r#"<div class="collapsible-content"><table><thead><tr><th class="num">独占 ms</th><th class="num">占比</th><th class="num">次数</th><th class="num">均值/次</th><th>函数名 [文件]</th></tr></thead><tbody>"#);

        let mut frame_funcs: std::collections::HashMap<u32, (f64, u32)> = std::collections::HashMap::new();
        for ev in &frame.events {
            if ev.self_ticks <= 0 { continue; }
            let entry = frame_funcs.entry(ev.desc_idx).or_insert((0.0, 0));
            entry.0 += stats::ticks_to_ms(ev.self_ticks, data.frequency);
            entry.1 += 1;
        }
        let mut top10: Vec<(u32, f64, u32)> = frame_funcs.into_iter().map(|(k, (ms, c))| (k, ms, c)).collect();
        top10.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        for (desc_idx, self_ms, count) in top10.iter().take(10) {
            let desc = &data.evt_descs[*desc_idx as usize];
            let p = if dur > 0.0 { self_ms / dur * 100.0 } else { 0.0 };
            let avg = if *count > 0 { self_ms / *count as f64 } else { 0.0 };
            let _ = write!(h, r#"<tr><td class="num">{:.1}</td><td class="num">{:.1}%</td><td class="num">{}</td><td class="num">{:.1}</td><td>{} [{}]</td></tr>"#,
                self_ms, p, count, avg, esc(&desc.name), esc(&stats::file_short(&desc.file)));
        }
        h.push_str("</tbody></table></div>");
    }
    h
}

fn section_hotspots(result: &AnalysisResult) -> String {
    let mut h = String::new();
    h.push_str(r#"<table><thead><tr><th class="num">#</th><th class="num">独占 ms</th><th class="num">占比</th><th>分布</th><th class="num">最大 ms</th><th class="num">次数</th><th class="num">均值 ms</th><th class="num">帧/总</th><th>函数名</th></tr></thead><tbody>"#);
    for (rank, f) in result.all_funcs.iter().take(30).enumerate() {
        let avg = if f.call_count > 0 { f.total_self / f.call_count as f64 } else { 0.0 };
        let pct = if result.total_slow_ms > 0.0 { f.total_self / result.total_slow_ms * 100.0 } else { 0.0 };
        let frame_str = format!("{}/{}", f.frame_count, result.slow_count);
        let _ = write!(h, r#"<tr><td class="num" data-sort="{}">{}</td><td class="num" data-sort="{:.1}">{:.1}</td><td class="num" data-sort="{:.1}">{:.1}%</td><td>{}</td><td class="num" data-sort="{:.1}">{:.1}</td><td class="num" data-sort="{}">{}</td><td class="num">{:.1}</td><td class="num">{}</td><td>{}</td></tr>"#,
            rank, rank + 1, f.total_self, f.total_self, pct, pct, css_bar(pct, "#e94560"),
            f.max_self, f.max_self, f.call_count, f.call_count, avg, frame_str, esc(&f.name));
    }
    h.push_str("</tbody></table>");
    h
}

fn section_modules(result: &AnalysisResult) -> String {
    let mut h = String::new();
    h.push_str(r#"<table><thead><tr><th class="num">#</th><th class="num">独占 ms</th><th class="num">占比</th><th>分布</th><th class="num">函数数</th><th>模块</th></tr></thead><tbody>"#);
    for (rank, (file, ms, func_n)) in result.top_modules.iter().take(15).enumerate() {
        let p = if result.total_slow_ms > 0.0 { ms / result.total_slow_ms * 100.0 } else { 0.0 };
        let _ = write!(h, r#"<tr><td class="num">{}</td><td class="num">{:.1}</td><td class="num">{:.1}%</td><td>{}</td><td class="num">{}</td><td>{}</td></tr>"#,
            rank + 1, ms, p, css_bar(p, "#3498db"), func_n, esc(file));
    }
    h.push_str("</tbody></table>");
    h
}

fn section_call_freq(result: &AnalysisResult) -> String {
    let mut h = String::new();
    h.push_str(r#"<table><thead><tr><th class="num">#</th><th class="num">总次数</th><th class="num">均值 ms</th><th class="num">总独占 ms</th><th>函数名</th></tr></thead><tbody>"#);
    for (rank, &idx) in result.by_call_count_indices.iter().take(15).enumerate() {
        let f = &result.all_funcs[idx];
        let avg = if f.call_count > 0 { f.total_self / f.call_count as f64 } else { 0.0 };
        let _ = write!(h, r#"<tr><td class="num">{}</td><td class="num" data-sort="{}">{}</td><td class="num">{:.3}</td><td class="num">{:.1}</td><td>{}</td></tr>"#,
            rank + 1, f.call_count, f.call_count, avg, f.total_self, esc(&f.name));
    }
    h.push_str("</tbody></table>");
    h
}

fn section_stability(result: &AnalysisResult) -> String {
    let mut h = String::new();
    h.push_str(r#"<table><thead><tr><th>函数名</th><th class="num">均值 ms</th><th class="num">标准差</th><th class="num">CV</th><th>评级</th></tr></thead><tbody>"#);
    for &(idx, m, s, cv) in result.stability.iter().take(15) {
        let f = &result.all_funcs[idx];
        let rating = if cv < 0.5 { r#"<span class="badge" style="background:#2ecc71">稳定</span>"# }
            else if cv < 1.0 { r#"<span class="badge" style="background:#f39c12">中等</span>"# }
            else { r#"<span class="badge" style="background:#e94560">不稳定</span>"# };
        let _ = write!(h, r#"<tr><td>{}</td><td class="num">{:.1}</td><td class="num">{:.1}</td><td class="num">{:.2}</td><td>{}</td></tr>"#,
            esc(&f.name), m, s, cv, rating);
    }
    h.push_str("</tbody></table>");
    h
}

fn section_trends(result: &AnalysisResult) -> String {
    let trend = &result.trend;
    let mut h = String::new();

    // ── 趋势 sparkline 表 ──
    h.push_str(r#"<table><thead><tr><th class="num">#</th><th>函数名</th><th>趋势图</th><th>模式</th><th class="num">均值 ms</th><th class="num">标准差</th><th class="num">CV</th><th class="num">出现率</th><th class="num">斜率 ms/帧</th><th class="num">最大 ms</th></tr></thead><tbody>"#);
    for (i, t) in trend.func_trends.iter().enumerate() {
        let color = pattern_color(&t.pattern);
        let _ = write!(h, r#"<tr><td class="num">{}</td><td title="{}">{}</td><td>{}</td><td>{}</td><td class="num">{:.2}</td><td class="num">{:.2}</td><td class="num">{:.2}</td><td class="num">{:.0}%</td><td class="num">{:+.3}</td><td class="num">{:.1}</td></tr>"#,
            i + 1, esc(&t.file), esc(&t.name),
            svg_sparkline(&t.sparkline, color, 120, 28),
            pattern_badge(&t.pattern),
            t.mean_ms, t.std_ms, t.cv,
            t.appearance_rate * 100.0,
            t.trend_slope, t.max_ms);
    }
    h.push_str("</tbody></table>");

    // ── 交叉函数相关性 ──
    if !trend.correlations.is_empty() {
        h.push_str(r#"<h3 style="margin-top:24px;color:var(--text2);font-size:15px">交叉函数相关性 (|Pearson| ≥ 0.7)</h3>"#);
        for c in trend.correlations.iter().take(20) {
            let cls = if c.pearson > 0.0 { "positive" } else { "negative" };
            let _ = write!(h, r#"<div class="correlation-pair"><div class="names">{} ↔ {}</div><div style="font-size:12px;color:var(--text2)">共现 {} 帧</div><div class="coeff {}">{:+.3}</div></div>"#,
                esc(&c.name_a), esc(&c.name_b), c.co_occurrence, cls, c.pearson);
        }
    }
    h
}

fn section_caller_callee(data: &ParsedData, result: &AnalysisResult) -> String {
    let mut h = String::new();
    for (rank, f) in result.all_funcs.iter().take(3).enumerate() {
        let _ = write!(h, r#"<div class="collapsible" style="padding:10px;background:var(--card2);border-radius:6px;margin-bottom:8px;font-weight:600">#{rank} {} [{}]</div>"#,
            esc(&f.name), esc(&stats::file_short(&f.file)));
        h.push_str(r#"<div class="collapsible-content"><div style="display:grid;grid-template-columns:1fr 1fr;gap:16px"><div>"#);

        // 调用者
        h.push_str("<strong>被以下函数调用:</strong><ul>");
        if let Some(callers) = result.caller_map.get(&f.desc_idx) {
            let mut sorted: Vec<(u32, u32)> = callers.iter().map(|(&k, &v)| (k, v)).collect();
            sorted.sort_unstable_by(|a, b| b.1.cmp(&a.1));
            for (parent_desc, count) in sorted.iter().take(5) {
                let pdesc = &data.evt_descs[*parent_desc as usize];
                let _ = write!(h, r#"<li>{}× {} [{}]</li>"#, count, esc(&pdesc.name), esc(&stats::file_short(&pdesc.file)));
            }
        } else {
            h.push_str("<li>(无调用者 — 顶层事件)</li>");
        }
        h.push_str("</ul></div><div>");

        // 被调用者
        h.push_str("<strong>调用了以下子函数 (Top 5):</strong><ul>");
        if let Some(callees) = result.callee_map.get(&f.desc_idx) {
            let mut sorted: Vec<(u32, u32)> = callees.iter().map(|(&k, &v)| (k, v)).collect();
            sorted.sort_unstable_by(|a, b| b.1.cmp(&a.1));
            for (child_desc, count) in sorted.iter().take(5) {
                let cdesc = &data.evt_descs[*child_desc as usize];
                let _ = write!(h, r#"<li>{}× {} [{}]</li>"#, count, esc(&cdesc.name), esc(&stats::file_short(&cdesc.file)));
            }
        }
        h.push_str("</ul></div></div></div>");
    }
    h
}

fn section_critical_path(result: &AnalysisResult) -> String {
    if result.critical_path.is_empty() { return String::new(); }
    let mut h = String::new();
    h.push_str(r#"<div class="call-path">"#);
    for (level, (name, file)) in result.critical_path.iter().enumerate() {
        let indent = "  ".repeat(level);
        let arrow = if level < result.critical_path.len() - 1 { "├─" } else { "└─" };
        let _ = write!(h, r#"<div><span class="level">{indent}{arrow}</span> {} [{}]</div>"#, esc(name), esc(file));
    }
    h.push_str(&format!("</div><p style='margin-top:8px;color:var(--text2)'>最大调用深度: {}</p>", result.max_depth));
    h
}

fn section_suggestions(data: &ParsedData, result: &AnalysisResult) -> String {
    let mut h = String::new();

    if let Some(top1) = result.all_funcs.first() {
        let pct = if result.total_slow_ms > 0.0 { top1.total_self / result.total_slow_ms * 100.0 } else { 0.0 };
        if pct > 50.0 {
            let _ = write!(h, r#"<div class="alert severe"><strong>[严重]</strong> '{}' 独占 {:.1}% 卡顿时间<br>出现在 {}/{} 卡顿帧，单帧最高 {:.0} ms<br>建议: 优先优化此函数，考虑异步化/分帧/并行化/减少计算量</div>"#,
                esc(&top1.name), pct, top1.frame_count, result.slow_count, top1.max_self);
        } else if pct > 20.0 {
            let _ = write!(h, r#"<div class="alert major"><strong>[主要]</strong> '{}' 占 {:.1}% 卡顿时间</div>"#, esc(&top1.name), pct);
        }
    }

    for &idx in result.by_call_count_indices.iter().take(3) {
        let f = &result.all_funcs[idx];
        if f.call_count > 1000 {
            let avg = f.total_self / f.call_count as f64;
            let _ = write!(h, r#"<div class="alert freq"><strong>[高频]</strong> '{}' 总调用 {} 次，平均 {:.3} ms/次<br>总独占 {:.1} ms — 考虑减少调用次数或批处理</div>"#,
                esc(&f.name), f.call_count, avg, f.total_self);
        }
    }

    for &(idx, m, s, cv) in result.stability.iter().take(3) {
        let f = &result.all_funcs[idx];
        if cv > 1.0 && f.per_frame_self.len() >= 3 {
            let _ = write!(h, r#"<div class="alert unstable"><strong>[不稳]</strong> '{}' 变异系数 {:.2} (均值 {:.1} ms, 标准差 {:.1} ms)<br>偶发性 spike — 排查触发条件 (数据量/缓存/锁竞争)</div>"#,
                esc(&f.name), cv, m, s);
        }
    }

    let slow_threads: Vec<&str> = result.slow_indices.iter()
        .map(|&i| data.frames[i].thread_name.as_str())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    if slow_threads.len() > 1 {
        let _ = write!(h, r#"<div class="alert thread"><strong>[多线程]</strong> {} 个线程出现卡顿帧: {}<br>检查线程间锁竞争或资源争用</div>"#,
            slow_threads.len(), slow_threads.join(", "));
    }

    if result.max_depth > 10 {
        let _ = write!(h, r#"<div class="alert depth"><strong>[调用链]</strong> 最慢帧最大调用深度 {} 层 — 考虑扁平化逻辑</div>"#, result.max_depth);
    }

    if h.is_empty() {
        h.push_str("<p style='color:var(--text2)'>未检测到明显性能问题。</p>");
    }
    h
}

// ── 主入口 ──

pub fn generate_html(data: &ParsedData, result: &AnalysisResult, input: &str) -> String {
    let mut html = String::with_capacity(64 * 1024);

    let _ = writeln!(html, r#"<!DOCTYPE html><html lang="zh"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"><title>Optick 性能分析报告 — {}</title><style>{}</style></head><body><div class="container">"#,
        esc(input), css());

    // Header
    let _ = writeln!(html, r#"<header><h1>Optick 性能分析报告</h1><div class="meta">文件: {} | {:.1} MB | 时钟: {:.1} MHz | 生成于本地</div></header>"#,
        esc(input), data.file_size_mb, data.frequency as f64 / 1e6);

    // Summary
    let _ = writeln!(html, r#"<section id="summary"><h2>概览</h2>{}</section>"#, section_summary(data, result, input));

    // Frame stats
    let _ = writeln!(html, r#"<section id="frame-stats"><h2>帧耗时统计</h2>{}</section>"#, section_frame_stats(result));

    // Frame budget
    let _ = writeln!(html, r#"<section id="budget"><h2>帧预算分析</h2>{}</section>"#, section_frame_budget(result));

    // Pareto
    if !section_pareto(result).is_empty() {
        let _ = writeln!(html, r#"<section id="pareto"><h2>热点集中度 (Pareto)</h2>{}</section>"#, section_pareto(result));
    }

    // Threads
    let _ = writeln!(html, r#"<section id="threads"><h2>线程分析</h2>{}</section>"#, section_threads(data, result));

    // Timeline
    let _ = writeln!(html, r#"<section id="timeline"><h2>帧时间线</h2>{}</section>"#, section_timeline(data, result));

    // Slow frames
    if result.slow_count > 0 {
        let _ = writeln!(html, r#"<section id="slow-frames"><h2>卡顿帧详情 (Top 5 最慢帧 — 点击展开)</h2>{}</section>"#, section_slow_frames(data, result));
    }

    // Hotspots
    let _ = writeln!(html, r#"<section id="hotspots"><h2>跨帧热点函数</h2>{}</section>"#, section_hotspots(result));

    // Modules
    let _ = writeln!(html, r#"<section id="modules"><h2>模块级聚合</h2>{}</section>"#, section_modules(result));

    // Call frequency
    let _ = writeln!(html, r#"<section id="call-freq"><h2>调用频次分析</h2>{}</section>"#, section_call_freq(result));

    // Stability
    let _ = writeln!(html, r#"<section id="stability"><h2>函数稳定性分析</h2>{}</section>"#, section_stability(result));

    // Trends
    let _ = writeln!(html, r#"<section id="trends"><h2>趋势分析</h2>{}</section>"#, section_trends(result));

    // Caller/Callee
    let _ = writeln!(html, r#"<section id="caller-callee"><h2>热点函数调用者分析 (Top 3 — 点击展开)</h2>{}</section>"#, section_caller_callee(data, result));

    // Critical path
    let cp = section_critical_path(result);
    if !cp.is_empty() {
        let _ = writeln!(html, r#"<section id="critical-path"><h2>关键路径</h2>{}</section>"#, cp);
    }

    // Suggestions
    let _ = writeln!(html, r#"<section id="suggestions"><h2>自动化优化建议</h2>{}</section>"#, section_suggestions(data, result));

    let _ = writeln!(html, r#"</div><script>{}</script></body></html>"#, js());
    html
}