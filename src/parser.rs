//! Optick .opt 二进制解析
//!
//! 基于 Optick 官方源码 (github.com/bombomby/optick) 的格式规范:
//! - 文件头: 8 字节 (magic + version + flags)
//! - 块头: 12 字节 (version=26, size, type, app=0xB50F)
//! - FDB (type 0): 顺序布局，无启发式扫描
//! - EventFrame (type 1): ScopeHeader + categories + events
//! - gzip/zlib 压缩由 main.rs 在调用前处理

use crate::binary;
use crate::model::*;

// ── 常量 ──

const MAGIC: u32 = 0xB50FB50F;
const NETWORK_PROTOCOL_VERSION: u32 = 26;
const NETWORK_APPLICATION_ID: u16 = 0xB50F;

// DataResponse::Type
const TYPE_FDB: u16 = 0;         // FrameDescriptionBoard
const TYPE_EVENT_FRAME: u16 = 1; // EventFrame (ScopeData)
const TYPE_NULL_FRAME: u16 = 3;  // 终止符

// ── 块结构 ──

pub fn parse_blocks(buf: &[u8]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut offset = 8; // 跳过 8 字节文件头
    while offset + 12 <= buf.len() {
        let ver = binary::u32(buf, offset);
        let size = binary::u32(buf, offset + 4);
        let type_ = binary::u16(buf, offset + 8);
        let app = binary::u16(buf, offset + 10);
        if ver != NETWORK_PROTOCOL_VERSION || app != NETWORK_APPLICATION_ID { break; }
        blocks.push(Block { type_, size, payload_start: offset + 12, payload_end: offset + 12 + size as usize });
        offset += 12 + size as usize;
        if type_ == TYPE_NULL_FRAME { break; }
    }
    blocks
}

// ── FDB 顺序解析 ──
// 官方布局 (DumpBoard):
//   boardNumber(u32) frequency(i64) origin(u64) precision(u32)
//   timeSlice.start(i64) timeSlice.finish(i64)
//   threads.count(u32) threads[]
//   fibers.count(u32) fibers[]
//   forcedMainThreadIndex(u32)
//   eventDescs.count(u32) eventDescs[]
//   tags(u32) run(u32) filters(u32) threadDescs1.count(u32) mode(u32)
//   processDescs.count(u32) processDescs[]
//   threadDescs2.count(u32) threadDescs2[]
//   processID(u32) hardware_concurrency(u32)
//
// ThreadDescription: threadID(u64) processID(u32) name(string) maxDepth(i32) priority(i32) mask(u32)
// EventDescription:  name(string) file(string) line(u32) filter(u32) color(u32) float(f32→4B) flags(u8)
// FiberDescription:  id(u64)
// ProcessDescription: processID(u32) name(string) uniqueKey(u64)

fn parse_thread_desc(buf: &[u8], pos: usize) -> (ThreadDesc, usize) {
    let _thread_id = binary::u64(buf, pos);          // threadID (8)
    let _process_id = binary::u32(buf, pos + 8);      // processID (4)
    let (name, next) = binary::read_string(buf, pos + 12); // name (string)
    // maxDepth(4) + priority(4) + mask(4) = 12 bytes after name
    (ThreadDesc { name }, next + 12)
}

fn parse_fiber_desc(buf: &[u8], pos: usize) -> (FiberDesc, usize) {
    let id = binary::u64(buf, pos);
    (FiberDesc { id }, pos + 8)
}

fn parse_event_desc(buf: &[u8], pos: usize) -> (EventDesc, usize) {
    let (name, pos) = binary::read_string(buf, pos);
    let (file, pos) = binary::read_string(buf, pos);
    let line = binary::u32(buf, pos);     // line (4)
    let _filter = binary::u32(buf, pos + 4); // filter (4)
    let _color = binary::u32(buf, pos + 8);  // color (4)
    let _float = binary::u32(buf, pos + 12); // float, always 0.0 (4)
    let _flags = buf[pos + 16];             // flags (1)
    (EventDesc { name, file, line }, pos + 17)
}

fn parse_fdb(buf: &[u8], ps: usize, _pe: usize) -> (Vec<ThreadDesc>, Vec<FiberDesc>, Vec<EventDesc>, u64, u32) {
    let mut pos = ps;

    // boardNumber (4) — 跳过
    pos += 4;

    // frequency (int64, 8 字节)
    let frequency = binary::i64(buf, pos) as u64;
    pos += 8;

    // origin (uint64, 8) + precision (uint32, 4) + timeSlice (i64+i64, 16)
    pos += 8 + 4 + 8 + 8;

    // threads
    let thread_count = binary::u32(buf, pos) as usize;
    pos += 4;
    let mut threads = Vec::with_capacity(thread_count);
    for _ in 0..thread_count {
        let (td, next) = parse_thread_desc(buf, pos);
        threads.push(td);
        pos = next;
    }

    // fibers
    let fiber_count = binary::u32(buf, pos) as usize;
    pos += 4;
    let mut fibers = Vec::with_capacity(fiber_count);
    for _ in 0..fiber_count {
        let (fd, next) = parse_fiber_desc(buf, pos);
        fibers.push(fd);
        pos = next;
    }

    // forcedMainThreadIndex (4)
    pos += 4;

    // eventDescs
    let evt_count = binary::u32(buf, pos);
    pos += 4;
    let mut evt_descs = Vec::with_capacity(evt_count as usize);
    for _ in 0..evt_count {
        let (ed, next) = parse_event_desc(buf, pos);
        evt_descs.push(ed);
        pos = next;
    }

    // 剩余字段不需要 (tags/run/filters/threadDescs1/mode/processDescs/threadDescs2/...)
    (threads, fibers, evt_descs, frequency, evt_count)
}

// ── 主解析入口 ──

pub fn parse(buf: &[u8], file_size_mb: f64) -> ParsedData {
    let magic = binary::u32(buf, 0);
    if magic != MAGIC { eprintln!("无效 Optick 文件"); std::process::exit(1); }

    // 压缩已在 main.rs 中处理，这里只处理未压缩数据
    let blocks = parse_blocks(buf);

    // ── FDB (type 0) ──
    let fdb = blocks.iter().find(|b| b.type_ == TYPE_FDB).expect("无 FDB 块");
    let (threads, fibers, evt_descs, frequency, evt_count) = parse_fdb(buf, fdb.payload_start, fdb.payload_end);

    // ── EventFrame (type 1) ──
    // ScopeHeader (32 bytes):
    //   boardNumber(u32) threadNumber(i32) fiberNumber(i32)
    //   event.start(i64) event.finish(i64) type(i32)
    // Body:
    //   categories.count(u32) categories[](×20) events.count(u32) events[](×20)
    let mut frames = Vec::new();
    for b in &blocks {
        if b.type_ != TYPE_EVENT_FRAME { continue; }
        let p = b.payload_start;
        let thread_number = binary::i32(buf, p + 4);
        let fiber_number = binary::i32(buf, p + 8);
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

        // 线程/Fiber 名称解析
        let thread_name = if thread_number >= 0 {
            threads.get(thread_number as usize)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| format!("T{}", thread_number))
        } else if fiber_number >= 0 {
            fibers.get(fiber_number as usize)
                .map(|f| format!("Fiber-{}", f.id))
                .unwrap_or_else(|| format!("Fiber{}", fiber_number))
        } else {
            "Unknown".into()
        };

        frames.push(Frame { thread_name, frame_start, frame_finish, events });
    }

    ParsedData { threads, fibers, evt_descs, frames, frequency, evt_count, file_size_mb }
}