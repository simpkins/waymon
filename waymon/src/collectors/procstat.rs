use crate::read::read_to_string_with_limit;
use std::path::Path;
use thiserror::Error;

const PROC_STAT_PATH: &str = "/proc/stat";
const MAX_NUM_CPUS: u64 = 1024 * 1024;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid integer")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("missing CPU ID")]
    NoCpuId,
    #[error("maximum CPU ID exceeded ")]
    MaxCpuCountExceeded,
    #[error("missing field in /proc/stat CPU line")]
    MissingCpuField,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Ticks(u64);

impl Ticks {
    pub fn value(&self) -> u64 {
        self.0
    }
}

impl std::ops::Add for Ticks {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self(self.0 + other.0)
    }
}

impl std::ops::Sub for Ticks {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Self(self.0 - other.0)
    }
}

impl std::ops::Div for Ticks {
    type Output = f64;

    fn div(self, other: Self) -> Self::Output {
        (self.0 as f64) / (other.0 as f64)
    }
}

#[derive(Debug, Clone)]
pub struct CpuStats {
    pub user: Ticks,
    pub nice: Ticks,
    pub system: Ticks,
    pub idle: Ticks,
    pub iowait: Ticks,
    pub irq: Ticks,
    pub softirq: Ticks,
    pub steal: Ticks,
    pub guest: Ticks,
    pub guest_nice: Ticks,
}

impl CpuStats {
    pub fn zero() -> CpuStats {
        CpuStats {
            user: Ticks(0),
            nice: Ticks(0),
            system: Ticks(0),
            idle: Ticks(0),
            iowait: Ticks(0),
            irq: Ticks(0),
            softirq: Ticks(0),
            steal: Ticks(0),
            guest: Ticks(0),
            guest_nice: Ticks(0),
        }
    }

    fn parse(&mut self, line: &str) -> Result<(), ParseError> {
        let mut iter = line.split(' ');

        // The first 4 fields should always be present
        self.user = Ticks(
            iter.next()
                .ok_or(ParseError::MissingCpuField)?
                .parse::<u64>()?,
        );
        self.nice = Ticks(
            iter.next()
                .ok_or(ParseError::MissingCpuField)?
                .parse::<u64>()?,
        );
        self.system = Ticks(
            iter.next()
                .ok_or(ParseError::MissingCpuField)?
                .parse::<u64>()?,
        );
        self.idle = Ticks(
            iter.next()
                .ok_or(ParseError::MissingCpuField)?
                .parse::<u64>()?,
        );

        // The remaining fields were added over time in various versions of Linux,
        // and so aren't guaranteed to be present if we are on a old kernel version.
        self.iowait = Ticks(iter.next().map_or(Ok(0), |x| x.parse::<u64>())?);
        self.irq = Ticks(iter.next().map_or(Ok(0), |x| x.parse::<u64>())?);
        self.softirq = Ticks(iter.next().map_or(Ok(0), |x| x.parse::<u64>())?);
        self.steal = Ticks(iter.next().map_or(Ok(0), |x| x.parse::<u64>())?);
        self.guest = Ticks(iter.next().map_or(Ok(0), |x| x.parse::<u64>())?);
        self.guest_nice = Ticks(iter.next().map_or(Ok(0), |x| x.parse::<u64>())?);

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ProcStat {
    pub cpu: CpuStats,
    pub cpus: Vec<CpuStats>,
    pub num_forks: u64,
    pub num_context_switches: u64,
    pub procs_running: u64,
    pub procs_blocked: u64,
    pub boot_time: u64, // boot time in seconds since the epoch
}

impl crate::stats::StatType for ProcStat {
    fn name() -> &'static str {
        PROC_STAT_PATH
    }

    fn new_zero() -> Self {
        Self {
            cpu: CpuStats::zero(),
            cpus: Vec::new(),
            num_forks: 0,
            num_context_switches: 0,
            procs_running: 0,
            procs_blocked: 0,
            boot_time: 0,
        }
    }

    fn update(&mut self) -> Result<(), crate::stats::StatsError> {
        *self = Self::read()?;
        Ok(())
    }
}

impl ProcStat {
    pub fn read() -> Result<ProcStat, std::io::Error> {
        let data = read_to_string_with_limit(Path::new(PROC_STAT_PATH), 1024 * 1024)?;
        Ok(Self::parse(&data))
    }

    pub fn parse(data: &str) -> ProcStat {
        let mut ps = ProcStat {
            cpu: CpuStats::zero(),
            cpus: Vec::new(),
            num_forks: 0,
            num_context_switches: 0,
            procs_running: 0,
            procs_blocked: 0,
            boot_time: 0,
        };

        for (_index, line) in data.split('\n').enumerate() {
            if let Err(_e) = ps.parse_line(line) {
                // We ignore errors parsing individual lines.
                // TODO: perhaps add a --verbose option to report warnings in the future, but in
                // general we don't want to spew lots of warnings on every stat update attempt.
                // eprintln!("{}:{} {:?}", PROC_STAT_PATH, _index + 1, _e);
            }
        }

        ps
    }

    fn parse_line(&mut self, line: &str) -> Result<(), ParseError> {
        if let Some(data) = line.strip_prefix("cpu") {
            self.parse_cpu_line(data)
        } else if let Some(data) = line.strip_prefix("btime ") {
            self.boot_time = data.parse::<u64>()?;
            Ok(())
        } else if let Some(data) = line.strip_prefix("ctxt ") {
            self.num_context_switches = data.parse::<u64>()?;
            Ok(())
        } else if let Some(data) = line.strip_prefix("processes ") {
            self.num_forks = data.parse::<u64>()?;
            Ok(())
        } else if let Some(data) = line.strip_prefix("procs_running ") {
            self.procs_running = data.parse::<u64>()?;
            Ok(())
        } else if let Some(data) = line.strip_prefix("procs_blocked ") {
            self.procs_blocked = data.parse::<u64>()?;
            Ok(())
        } else {
            // We currently don't bother parsing the intr or softirq lines
            // Other lines may be added in the future by newer kernel versions
            Ok(())
        }
    }

    fn parse_cpu_line(&mut self, line: &str) -> Result<(), ParseError> {
        if let Some(data) = line.strip_prefix("  ") {
            self.cpu.parse(data)
        } else {
            let (cpu_id_str, data) = line.split_once(' ').ok_or(ParseError::NoCpuId)?;
            let cpu_id = cpu_id_str.parse::<u64>()?;
            if cpu_id > MAX_NUM_CPUS {
                // Avoid allocating a huge array if we get a crazy CPU number
                return Err(ParseError::MaxCpuCountExceeded);
            }
            let cpu_index = cpu_id as usize;
            if cpu_index >= self.cpus.len() {
                self.cpus.resize_with(cpu_index + 1, || CpuStats::zero())
            }
            self.cpus[cpu_index].parse(data)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let ps = ProcStat::parse(TEST_DATA);
        assert_eq!(ps.boot_time, 1698786578);
        assert_eq!(ps.num_forks, 11536992);
        assert_eq!(ps.procs_running, 2);
        assert_eq!(ps.procs_blocked, 1);
        assert_eq!(ps.num_context_switches, 28454511446);

        assert_eq!(ps.cpu.user, Ticks(66570360));
        assert_eq!(ps.cpu.nice, Ticks(34454637));
        assert_eq!(ps.cpu.system, Ticks(59761125));
        assert_eq!(ps.cpu.idle, Ticks(2564509174));
        assert_eq!(ps.cpu.iowait, Ticks(1202763));
        assert_eq!(ps.cpu.irq, Ticks(1));
        assert_eq!(ps.cpu.softirq, Ticks(2336491));
        assert_eq!(ps.cpu.steal, Ticks(2));
        assert_eq!(ps.cpu.guest, Ticks(3));
        assert_eq!(ps.cpu.guest_nice, Ticks(4));

        assert_eq!(ps.cpus.len(), 12);
        let cpu0 = &ps.cpus[0];
        assert_eq!(cpu0.user, Ticks(6535178));
        assert_eq!(cpu0.nice, Ticks(4878199));
        assert_eq!(cpu0.system, Ticks(5230345));
        assert_eq!(cpu0.idle, Ticks(492195700));
        assert_eq!(cpu0.iowait, Ticks(702649));
        assert_eq!(cpu0.irq, Ticks(0));
        assert_eq!(cpu0.softirq, Ticks(1782748));
        assert_eq!(cpu0.steal, Ticks(0));
        assert_eq!(cpu0.guest, Ticks(0));
        assert_eq!(cpu0.guest_nice, Ticks(0));

        let cpu11 = &ps.cpus[11];
        assert_eq!(cpu11.user, Ticks(2063384));
        assert_eq!(cpu11.nice, Ticks(1390869));
        assert_eq!(cpu11.system, Ticks(977731));
        assert_eq!(cpu11.idle, Ticks(193159395));
        assert_eq!(cpu11.iowait, Ticks(41631));
        assert_eq!(cpu11.irq, Ticks(0));
        assert_eq!(cpu11.softirq, Ticks(13830));
        assert_eq!(cpu11.steal, Ticks(0));
        assert_eq!(cpu11.guest, Ticks(0));
        assert_eq!(cpu11.guest_nice, Ticks(0));
    }

    const TEST_DATA: &str = r#"
cpu  66570360 34454637 59761125 2564509174 1202763 1 2336491 2 3 4
cpu0 6535178 4878199 5230345 492195700 702649 0 1782748 0 0 0
cpu1 5795582 4332918 3563030 189017303 54923 0 159171 0 0 0
cpu2 8687275 5608893 11399048 180274904 75490 0 98886 0 0 0
cpu3 3267223 2306442 1719108 192590868 35981 0 98303 0 0 0
cpu4 6124358 4092894 4323710 188341546 53006 0 55803 0 0 0
cpu5 4726787 3030214 3127342 190186208 51955 0 37262 0 0 0
cpu6 9488527 2129654 6652400 184996241 29027 0 24922 0 0 0
cpu7 12092327 1565491 18701166 175881216 23347 0 17537 0 0 0
cpu8 2918198 1888462 1666184 192253656 45882 0 18786 0 0 0
cpu9 2567712 1696679 1300400 192671939 46024 0 15374 0 0 0
cpu10 2303806 1533918 1100657 192940193 42844 0 13865 0 0 0
cpu11 2063384 1390869 977731 193159395 41631 0 13830 0 0 0
intr 8206296170 0 158678 0 0 0 0 0 0 0 5892497 0 0 643 0 264916 0 0 0 0 0 2 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 2090454 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 6 1 0 91 204733398 18575 18502 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 264916 29 626239 464665 776204 430650 606502 537061 494925 435965 556124 567567 540687 572805 230936550 97318143 10284158 15865598 17451346 14818682 12351648 7351017 8243654 9207788 4634878 5930402 14307051 2810614 185 1177 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
ctxt 28454511446
btime 1698786578
processes 11536992
procs_running 2
procs_blocked 1
softirq 5594134747 187873945 250846610 1837 2554548318 10570578 1001 47353921 1594949128 568785 947420624
"#;
}

// The test data has a really long line, which is longer than vim's default limits for syntax
// highlight processing.  Increase the limit, otherwise vim misses the end of this string literal,
// making it think that subsequent lines are still part of a string literal.
// vim: synmaxcol=5000
