//! 调用树重建 + selfTime 计算
//!
//! 事件已按 (start ASC, finish DESC) 排序 → 父在子前
//! 栈算法: 弹出已结束的栈顶 → 剩余栈顶 = 父
//! selfTime = duration - sum(direct children duration)

use crate::model::Frame;

pub fn build_call_trees(frames: &mut [Frame]) {
    for frame in frames.iter_mut() {
        let evs = &mut frame.events;
        let mut stack: Vec<usize> = Vec::new();
        for i in 0..evs.len() {
            evs[i].self_ticks = evs[i].finish - evs[i].start;
            while let Some(&top) = stack.last() {
                if evs[top].finish <= evs[i].start { stack.pop(); } else { break; }
            }
            if let Some(&pidx) = stack.last() {
                evs[i].parent_idx = Some(pidx);
                evs[pidx].self_ticks -= evs[i].finish - evs[i].start;
            }
            stack.push(i);
        }
    }
}