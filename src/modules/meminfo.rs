use std::fmt::Write;

/// Parsed fields from /proc/meminfo
pub struct MemInfo {
    pub mem_total: u64,
    pub mem_available: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}

/// Read and parse /proc/meminfo once for both memory and swap modules
pub fn read_meminfo() -> Option<MemInfo> {
    let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut mem_total: u64 = 0;
    let mut mem_available: u64 = 0;
    let mut swap_total: u64 = 0;
    let mut swap_free: u64 = 0;

    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            mem_total = parse_kb(rest)?;
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            mem_available = parse_kb(rest)?;
        } else if let Some(rest) = line.strip_prefix("SwapTotal:") {
            swap_total = parse_kb(rest)?;
        } else if let Some(rest) = line.strip_prefix("SwapFree:") {
            swap_free = parse_kb(rest)?;
        }
    }

    Some(MemInfo {
        mem_total,
        mem_available,
        swap_total,
        swap_free,
    })
}

fn parse_kb(s: &str) -> Option<u64> {
    s.trim()
        .trim_end_matches("kB")
        .trim()
        .parse::<u64>()
        .ok()
}

pub fn format_bytes_into(buf: &mut String, bytes: u64) {
    let gib = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gib >= 1.0 {
        let _ = write!(buf, "{gib:.1} GiB");
    } else {
        let mib = bytes as f64 / (1024.0 * 1024.0);
        let _ = write!(buf, "{mib:.0} MiB");
    }
}
