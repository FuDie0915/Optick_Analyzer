//! Optick .opt 二进制解析

use crate::binary;
use crate::model::*;

pub fn parse_blocks(buf: &[u8]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut offset = 8;
    while offset + 12 <= buf.len() {
        let ver = binary::u32(buf, offset);
        let size = binary::u32(buf, offset + 4);
        let type_ = binary::u16(buf, offset + 8);
        let app = binary::u16(buf, offset + 10);
        if ver != 26 || app != 0xB50F { break; }
        blocks.push(Block { type_, size, payload_start: offset + 12, payload_end: offset + 12 + size as usize });
        offset += 12 + size as usize;
        if type_ == 3 { break; }
    }
    blocks
}

pub fn parse(buf: &[u8]) -> ParsedData {
    let magic = binary::u32(buf, 0);
    if magic != 0xB50FB50F { eprintln!("无效 Optick 文件"); std::process::exit(1); }

    let file_size_mb = buf.len() as f64 / 1_048_576.0;
    let blocks = parse_blocks(buf);

    // ── FrameDescriptionBoard (type 0) ──
    let fdb = blocks.iter().find(|b| b.type_ == 0).expect("无 FDB 块");
    let ps = fdb.payload_start;
    let pe = fdb.payload_end;
    let frequency = binary::u32(buf, ps + 4) as u64;

    // 定位线程区: 搜索已知线程名
    let known = ["State Thread", "LogicThread", "Parallel JobSystem", "Frame"];
    let mut thread_start = usize::MAX;
    for name in &known {
        let nb = name.as_bytes();
        let mut needle = Vec::with_capacity(4 + nb.len());
        needle.extend_from_slice(&(nb.len() as u32).to_le_bytes());
        needle.extend_from_slice(nb);
        if let Some(idx) = binary::find_bytes(buf, &needle, ps) {
            if idx < pe && idx >= ps + 12 {
                let c = idx - 12;
                if c < thread_start { thread_start = c; }
            }
        }
    }
    if thread_start == usize::MAX { panic!("无法定位线程区"); }
    let thread_count = binary::u32(buf, thread_start - 4) as usize;

    // 解析线程
    let mut pos = thread_start;
    let mut threads = Vec::with_capacity(thread_count);
    for _ in 0..thread_count {
        pos += 4; // maxDepth
        pos += 4; // priority
        pos += 4; // mask
        let (name, next) = binary::read_string(buf, pos); pos = next;
        pos += 8; // threadID
        pos += 4; // processID
        threads.push(ThreadDesc { name });
    }

    // 定位事件描述区
    let mut evt_desc_start = 0;
    let mut evt_desc_count = 0u32;
    for skip in (0..=16).step_by(4) {
        let count = binary::u32(buf, pos + skip);
        if count > 0 && count < 100000 {
            let nl = binary::u32(buf, pos + skip + 4);
            if nl > 0 && nl < 500 {
                let pv_start = pos + skip + 8;
                if pv_start < buf.len() {
                    let first = buf[pv_start];
                    if first.is_ascii_alphanumeric() || first == b'_' || first == b'(' || first == b':' {
                        evt_desc_count = count;
                        evt_desc_start = pos + skip + 4;
                        break;
                    }
                }
            }
        }
    }

    // 解析事件描述
    pos = evt_desc_start;
    let mut evt_descs = Vec::with_capacity(evt_desc_count as usize);
    for _ in 0..evt_desc_count {
        let (name, next) = binary::read_string(buf, pos); pos = next;
        let (file, next) = binary::read_string(buf, pos); pos = next;
        let line = binary::u32(buf, pos); pos += 4;
        pos += 4; // index
        pos += 4; // color
        pos += 4; // filter
        pos += 1; // flags
        evt_descs.push(EventDesc { name, file, line });
    }

    // ── EventFrame (type 1) ──
    let mut frames = Vec::new();
    for b in &blocks {
        if b.type_ != 1 { continue; }
        let p = b.payload_start;
        let thread_number = binary::i32(buf, p + 4);
        let frame_start = binary::i64(buf, p + 12);
        let frame_finish = binary::i64(buf, p + 20);
        let cat_count = binary::u32(buf, p + 32) as usize;
        let mut ep = p + 36 + cat_count * 20;
        let ev_count = binary::u32(buf, ep) as usize;
        ep += 4;

        let mut events = Vec::with_capacity(ev_count);
        for _ in 0..ev_count {
            events.push(Event {
                start: binary::i64(buf, ep),
                finish: binary::i64(buf, ep + 8),
                desc_idx: binary::u32(buf, ep + 16),
                self_ticks: 0,
                parent_idx: None,
            });
            ep += 20;
        }

        let thread_name = threads.get(thread_number as usize)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| format!("T{}", thread_number));

        frames.push(Frame { thread_name, frame_start, frame_finish, events });
    }

    ParsedData { threads, evt_descs, frames, frequency, evt_count: evt_desc_count, file_size_mb }
}