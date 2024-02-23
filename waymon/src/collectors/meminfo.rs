use crate::read::read_to_string_with_limit;
use std::path::Path;
use thiserror::Error;
use tracing::error;

const PATH: &str = "/proc/meminfo";

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid integer")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("unexpected data in /proc/meminfo")]
    UnexpectedData,
}

/**
 * Note: all values in the MemoryStats structure are in kilobytes, not bytes
 */
#[derive(Clone, Debug, Default)]
pub struct MemoryStats {
    pub mem_total: u64,
    pub mem_free: u64,
    pub mem_available: u64,
    pub buffers: u64,
    pub cached: u64,
    pub swap_cached: u64,
    pub active: u64,
    pub inactive: u64,
    pub unevictable: u64,
    pub mlocked: u64,
    pub high_total: u64,
    pub high_free: u64,
    pub low_total: u64,
    pub low_free: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub dirty: u64,
    pub writeback: u64,
    pub anon_pages: u64,
    pub mapped: u64,
    pub shmem: u64,
    pub kreclaimable: u64,
    pub slab: u64,
    pub sreclaimable: u64,
    pub sunreclaimable: u64,
    pub kernel_stack: u64,
    pub page_tables: u64,
    pub commit_limit: u64,
    pub committed_as: u64,
    pub vmalloc_total: u64,
    pub vmalloc_used: u64,
    // There are several other fields in /proc/meminfo that we don't currently bother tracking
}

impl MemoryStats {
    pub fn read() -> Result<Self, std::io::Error> {
        let data = read_to_string_with_limit(Path::new(PATH), 10 * 1024 * 1024)?;
        Ok(Self::parse(&data))
    }

    pub fn parse(data: &str) -> Self {
        let mut m: Self = Default::default();
        for (index, line) in data.split('\n').enumerate() {
            if let Err(e) = m.parse_line(line) {
                static PARSE_ERROR_LOG: std::sync::Once = std::sync::Once::new();
                PARSE_ERROR_LOG.call_once(|| {
                    error!("{}:{} {:?}", PATH, index + 1, e);
                });
            }
        }
        m
    }

    fn parse_line(&mut self, line: &str) -> Result<(), ParseError> {
        let _parsed = Self::try_parse_kb(line, "MemTotal:", &mut self.mem_total)?
            || Self::try_parse_kb(line, "MemFree:", &mut self.mem_free)?
            || Self::try_parse_kb(line, "MemAvailable:", &mut self.mem_available)?
            || Self::try_parse_kb(line, "Buffers:", &mut self.buffers)?
            || Self::try_parse_kb(line, "Cached:", &mut self.cached)?
            || Self::try_parse_kb(line, "SwapCached:", &mut self.swap_cached)?
            || Self::try_parse_kb(line, "Active:", &mut self.active)?
            || Self::try_parse_kb(line, "Inactive:", &mut self.inactive)?
            || Self::try_parse_kb(line, "Unevictable:", &mut self.unevictable)?
            || Self::try_parse_kb(line, "Mlocked:", &mut self.mlocked)?
            || Self::try_parse_kb(line, "HighTotal:", &mut self.high_total)?
            || Self::try_parse_kb(line, "HighFree:", &mut self.high_free)?
            || Self::try_parse_kb(line, "LowTotal:", &mut self.low_total)?
            || Self::try_parse_kb(line, "LowFree:", &mut self.low_free)?
            || Self::try_parse_kb(line, "SwapTotal:", &mut self.swap_total)?
            || Self::try_parse_kb(line, "SwapFree:", &mut self.swap_free)?
            || Self::try_parse_kb(line, "Dirty:", &mut self.dirty)?
            || Self::try_parse_kb(line, "Writeback:", &mut self.writeback)?
            || Self::try_parse_kb(line, "AnonPages:", &mut self.anon_pages)?
            || Self::try_parse_kb(line, "Mapped:", &mut self.mapped)?
            || Self::try_parse_kb(line, "Shmem:", &mut self.shmem)?
            || Self::try_parse_kb(line, "KReclaimable:", &mut self.kreclaimable)?
            || Self::try_parse_kb(line, "Slab:", &mut self.slab)?
            || Self::try_parse_kb(line, "SReclaimable:", &mut self.sreclaimable)?
            || Self::try_parse_kb(line, "SUnreclaim:", &mut self.sunreclaimable)?
            || Self::try_parse_kb(line, "KernelStack:", &mut self.kernel_stack)?
            || Self::try_parse_kb(line, "PageTables:", &mut self.page_tables)?
            || Self::try_parse_kb(line, "CommitLimit:", &mut self.commit_limit)?
            || Self::try_parse_kb(line, "Committed_AS:", &mut self.committed_as)?
            || Self::try_parse_kb(line, "VmallocTotal:", &mut self.vmalloc_total)?
            || Self::try_parse_kb(line, "VmallocUsed:", &mut self.vmalloc_used)?;
        Ok(())
    }

    fn try_parse_kb(line: &str, prefix: &str, field: &mut u64) -> Result<bool, ParseError> {
        if let Some(data) = line.strip_prefix(prefix) {
            let data = data.trim_start();
            if let Some(data) = data.strip_suffix(" kB") {
                *field = data.parse()?;
                Ok(true)
            } else {
                Err(ParseError::UnexpectedData)
            }
        } else {
            Ok(false)
        }
    }
}

impl crate::stats::StatType for MemoryStats {
    fn name() -> &'static str {
        PATH
    }

    fn new_zero() -> Self {
        Default::default()
    }

    fn update(&mut self) -> Result<(), crate::stats::StatsError> {
        *self = Self::read()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_parse() -> Result<()> {
        let m = MemoryStats::parse(TEST_DATA);

        assert_eq!(m.mem_total, 24452240);
        assert_eq!(m.mem_free, 1107100);
        assert_eq!(m.mem_available, 13149848);
        assert_eq!(m.buffers, 1396164);
        assert_eq!(m.cached, 14007088);
        assert_eq!(m.swap_cached, 0);
        assert_eq!(m.active, 12562612);
        assert_eq!(m.inactive, 8298432);
        assert_eq!(m.unevictable, 807208);
        assert_eq!(m.mlocked, 160);
        assert_eq!(m.high_total, 0);
        assert_eq!(m.high_free, 0);
        assert_eq!(m.low_total, 0);
        assert_eq!(m.low_free, 0);
        assert_eq!(m.swap_total, 20971000);
        assert_eq!(m.swap_free, 20970232);
        assert_eq!(m.dirty, 60);
        assert_eq!(m.writeback, 0);
        assert_eq!(m.anon_pages, 6193256);
        assert_eq!(m.mapped, 798028);
        assert_eq!(m.shmem, 2260328);
        assert_eq!(m.kreclaimable, 817816);
        assert_eq!(m.slab, 1077340);
        assert_eq!(m.sreclaimable, 817816);
        assert_eq!(m.sunreclaimable, 259524);
        assert_eq!(m.kernel_stack, 26640);
        assert_eq!(m.page_tables, 65708);
        assert_eq!(m.commit_limit, 33197120);
        assert_eq!(m.committed_as, 16888952);
        assert_eq!(m.vmalloc_total, 34359738367);
        assert_eq!(m.vmalloc_used, 179892);

        Ok(())
    }

    const TEST_DATA: &str = r#"
MemTotal:       24452240 kB
MemFree:         1107100 kB
MemAvailable:   13149848 kB
Buffers:         1396164 kB
Cached:         14007088 kB
SwapCached:            0 kB
Active:         12562612 kB
Inactive:        8298432 kB
Active(anon):    7716384 kB
Inactive(anon):     1736 kB
Active(file):    4846228 kB
Inactive(file):  8296696 kB
Unevictable:      807208 kB
Mlocked:             160 kB
SwapTotal:      20971000 kB
SwapFree:       20970232 kB
Zswap:                 0 kB
Zswapped:              0 kB
Dirty:                60 kB
Writeback:             0 kB
AnonPages:       6193256 kB
Mapped:           798028 kB
Shmem:           2260328 kB
KReclaimable:     817816 kB
Slab:            1077340 kB
SReclaimable:     817816 kB
SUnreclaim:       259524 kB
KernelStack:       26640 kB
PageTables:        65708 kB
SecPageTables:         0 kB
NFS_Unstable:          0 kB
Bounce:                0 kB
WritebackTmp:          0 kB
CommitLimit:    33197120 kB
Committed_AS:   16888952 kB
VmallocTotal:   34359738367 kB
VmallocUsed:      179892 kB
VmallocChunk:          0 kB
Percpu:            14688 kB
HardwareCorrupted:     0 kB
AnonHugePages:    499712 kB
ShmemHugePages:  1910784 kB
ShmemPmdMapped:        0 kB
FileHugePages:         0 kB
FilePmdMapped:         0 kB
Unaccepted:            0 kB
HugePages_Total:       0
HugePages_Free:        0
HugePages_Rsvd:        0
HugePages_Surp:        0
Hugepagesize:       2048 kB
Hugetlb:               0 kB
DirectMap4k:      444092 kB
DirectMap2M:    13027328 kB
DirectMap1G:    11534336 kB
"#;
}
