use crate::read::read_to_string_with_limit;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

const PATH: &str = "/proc/diskstats";

// The sector sizes reported in /proc/diskstats are always in units of
// 512 bytes, regardless of the actual sector size used by the physical disk.
// See https://lkml.org/lkml/2015/8/17/269
pub const BYTES_PER_SECTOR: u64 = 512;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid integer")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("missing field in /proc/diskstats line")]
    MissingField,
}

#[derive(Debug, Clone)]
pub struct DiskStats {
    pub num_reads: u64,        // number of reads completed successfully
    pub num_reads_merged: u64, // number of reads operations merged
    pub num_sectors_read: u64,

    pub num_writes: u64,        // number of writes completed successfully
    pub num_writes_merged: u64, // number of write operations merged
    pub num_sectors_written: u64,

    pub num_discards: u64,
    pub num_discards_merged: u64,
    pub num_sectors_discarded: u64,

    pub num_flushes: u64,

    // The following fields are documented as unsigned int in the Linux kernel.
    // In practice this corresponds to u32 on most platforms we care about.
    // (We perhaps could store them as u64 anyway, at the minor expense of using more memory than
    // needed.)
    //
    pub ms_reading: u32,       // number of milliseconds spent reading
    pub ms_writing: u32,       // number of milliseconds spent writing
    pub iops_in_progress: u32, // number of I/O operations in progress
    pub ms_doing_io: u32,      // number of milliseconds spent doing I/O
    pub weighted_ms_doing_io: u32,
    pub ms_discarding: u32, // number of milliseconds spent doing discards
    pub ms_flushing: u32,   // number of milliseconds spent doing discards
}

impl DiskStats {
    pub fn zero() -> DiskStats {
        DiskStats {
            num_reads: 0,
            num_reads_merged: 0,
            num_sectors_read: 0,
            num_writes: 0,
            num_writes_merged: 0,
            num_sectors_written: 0,
            num_discards: 0,
            num_discards_merged: 0,
            num_sectors_discarded: 0,
            num_flushes: 0,
            ms_reading: 0,
            ms_writing: 0,
            iops_in_progress: 0,
            ms_doing_io: 0,
            weighted_ms_doing_io: 0,
            ms_discarding: 0,
            ms_flushing: 0,
        }
    }

    fn parse(line: &str) -> Result<DiskStats, ParseError> {
        let mut iter = line.split(' ');
        let mut d = Self::zero();

        // The first set of fields should be present since /proc/diskstats was first
        // added in ~2008.
        d.num_reads = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u64>()?;
        d.num_reads_merged = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u64>()?;
        d.num_sectors_read = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u64>()?;
        d.ms_reading = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u32>()?;

        d.num_writes = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u64>()?;
        d.num_writes_merged = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u64>()?;
        d.num_sectors_written = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u64>()?;
        d.ms_writing = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u32>()?;

        d.iops_in_progress = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u32>()?;
        d.ms_doing_io = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u32>()?;
        d.weighted_ms_doing_io = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u32>()?;

        // The following fields were added in Linux 4.18
        d.num_discards = iter.next().map_or(Ok(0), |x| x.parse::<u64>())?;
        d.num_discards_merged = iter.next().map_or(Ok(0), |x| x.parse::<u64>())?;
        d.num_sectors_discarded = iter.next().map_or(Ok(0), |x| x.parse::<u64>())?;
        d.ms_discarding = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u32>()?;

        // The following fields were added in Linux 5.5
        d.num_flushes = iter.next().map_or(Ok(0), |x| x.parse::<u64>())?;
        d.ms_flushing = iter
            .next()
            .ok_or(ParseError::MissingField)?
            .parse::<u32>()?;

        Ok(d)
    }
}

#[derive(Debug, Clone)]
pub struct ProcDiskStats {
    pub disks: HashMap<String, DiskStats>,
}

impl ProcDiskStats {
    pub fn read() -> Result<ProcDiskStats, std::io::Error> {
        let data = read_to_string_with_limit(Path::new(PATH), 10 * 1024 * 1024)?;
        Ok(Self::parse(&data))
    }

    pub fn parse(data: &str) -> ProcDiskStats {
        let mut d = ProcDiskStats {
            disks: HashMap::new(),
        };

        for (_index, line) in data.split('\n').enumerate() {
            if let Err(_e) = d.parse_line(line) {
                // We ignore errors parsing individual lines.
                // TODO: perhaps add a --verbose option to report warnings in the future, but in
                // general we don't want to spew lots of warnings on every stat update attempt.
                // eprintln!("{}:{} {:?}", PATH, _index + 1, _e);
            }
        }

        d
    }

    fn parse_line(&mut self, line: &str) -> Result<(), ParseError> {
        if line.is_empty() {
            // This happens after the final newline in the file.
            return Ok(());
        }

        let line = line.trim_start();
        let (_dev_major_str, line) = line.split_once(' ').ok_or(ParseError::MissingField)?;
        let line = line.trim_start();
        let (_dev_minor_str, line) = line.split_once(' ').ok_or(ParseError::MissingField)?;
        let line = line.trim_start();
        let (name, line) = line.split_once(' ').ok_or(ParseError::MissingField)?;

        self.disks.insert(name.to_string(), DiskStats::parse(line)?);
        Ok(())
    }
}

impl crate::stats::StatType for ProcDiskStats {
    fn name() -> &'static str {
        PATH
    }

    fn new_zero() -> Self {
        Self {
            disks: HashMap::new(),
        }
    }

    fn update(&mut self) -> Result<(), crate::stats::StatsError> {
        *self = Self::read()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{anyhow, Result};

    #[test]
    fn test_parse() -> Result<()> {
        let d = ProcDiskStats::parse(TEST_DATA);

        assert_eq!(d.disks.len(), 20);

        let ssd = d.disks.get("nvme0n1").ok_or(anyhow!("missing nvme0n1"))?;
        assert_eq!(ssd.num_reads, 342448);
        assert_eq!(ssd.num_reads_merged, 55631);
        assert_eq!(ssd.num_sectors_read, 31849342);
        assert_eq!(ssd.ms_reading, 46771);
        assert_eq!(ssd.num_writes, 1185451);
        assert_eq!(ssd.num_writes_merged, 633019);
        assert_eq!(ssd.num_sectors_written, 86064386);
        assert_eq!(ssd.ms_writing, 3770341);
        assert_eq!(ssd.iops_in_progress, 3);
        assert_eq!(ssd.ms_doing_io, 1776084);
        assert_eq!(ssd.weighted_ms_doing_io, 3981375);
        assert_eq!(ssd.num_discards, 5);
        assert_eq!(ssd.num_discards_merged, 2);
        assert_eq!(ssd.num_sectors_discarded, 3338440);
        assert_eq!(ssd.ms_discarding, 1);
        assert_eq!(ssd.num_flushes, 98812);
        assert_eq!(ssd.ms_flushing, 164259);

        Ok(())
    }

    const TEST_DATA: &str = r#"
   7       0 loop0 905 0 11080 1654 213 0 1744 583 0 640 2483 0 0 0 0 45 244
   7       1 loop1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
   7       2 loop2 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
   7       3 loop3 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
   7       4 loop4 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
   7       5 loop5 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
   7       6 loop6 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
   7       7 loop7 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 259       0 nvme0n1 342448 55631 31849342 46771 1185451 633019 86064386 3770341 3 1776084 3981375 5 2 3338440 1 98812 164259
 259       1 nvme0n1p1 138 1090 6914 32 1 0 1 8 0 80 42 3 0 138648 1 0 0
 259       2 nvme0n1p2 221 16369 21218 54 1 0 1 8 0 100 64 2 0 3199792 0 0 0
 259       3 nvme0n1p3 341713 38172 31801185 46634 1185447 633019 86064376 3770322 0 1776016 3816957 0 0 0 0 0 0
 259       4 nvme0n1p4 279 0 15105 34 2 0 8 1 0 76 35 0 0 0 0 0 0
   8       0 sda 13921 11329 1363808 242148 60 15 432 6106 0 78608 248657 0 0 0 0 42 402
 252       0 dm-0 379820 0 31799070 83416 1788027 0 86064376 8040516 0 1785748 8123932 0 0 0 0 0 0
 252       1 dm-1 379776 0 31797234 83752 1771147 0 86064376 7787540 0 1786268 7871292 0 0 0 0 0 0
 252       2 dm-2 192 0 8352 36 2 0 8 4 0 36 40 0 0 0 0 0 0
 251       0 zram0 53 0 2352 0 6 0 48 0 0 16 0 0 0 0 0 0 0
 252       3 dm-3 873 0 10760 1840 213 0 1744 492 0 628 2332 0 0 0 0 0 0
 252       4 dm-4 25151 0 1361920 469280 75 0 432 17436 0 69356 486716 0 0 0 0 0 0
"#;
}
