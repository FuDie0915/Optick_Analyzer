//! 二进制读取原语 (little-endian)

pub fn u32(b: &[u8], o: usize) -> u32 { u32::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3]]) }
pub fn i32(b: &[u8], o: usize) -> i32 { i32::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3]]) }
pub fn u16(b: &[u8], o: usize) -> u16 { u16::from_le_bytes([b[o],b[o+1]]) }
pub fn i64(b: &[u8], o: usize) -> i64 { i64::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3],b[o+4],b[o+5],b[o+6],b[o+7]]) }
#[allow(dead_code)]
pub fn u64(b: &[u8], o: usize) -> u64 { u64::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3],b[o+4],b[o+5],b[o+6],b[o+7]]) }

pub fn read_string(b: &[u8], o: usize) -> (String, usize) {
    let len = u32(b, o) as usize;
    let s = String::from_utf8_lossy(&b[o+4..o+4+len]).to_string();
    (s, o + 4 + len)
}

pub fn find_bytes(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from + needle.len() > hay.len() { return None; }
    for i in from..=hay.len() - needle.len() {
        if &hay[i..i+needle.len()] == needle { return Some(i); }
    }
    None
}