use crate::read::read_to_string_with_limit;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

const PATH: &str = "/proc/net/dev";

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid integer")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("missing field in /proc/net/dev line")]
    MissingField,
}

#[derive(Clone, Debug, Default)]
pub struct InterfaceStats {
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errs: u64,
    pub rx_drop: u64,
    pub rx_fifo: u64,
    pub rx_frame: u64,
    pub rx_compressed: u64,
    pub rx_multicast: u64,

    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errs: u64,
    pub tx_drop: u64,
    pub tx_fifo: u64,
    pub tx_colls: u64,
    pub tx_carrier: u64,
    pub tx_compressed: u64,
}

impl InterfaceStats {
    pub fn zero() -> InterfaceStats {
        Default::default()
    }

    fn parse(line: &str) -> Result<InterfaceStats, ParseError> {
        let mut s = Self::zero();
        let fields = [
            &mut s.rx_bytes,
            &mut s.rx_packets,
            &mut s.rx_errs,
            &mut s.rx_drop,
            &mut s.rx_fifo,
            &mut s.rx_frame,
            &mut s.rx_compressed,
            &mut s.rx_multicast,
            &mut s.tx_bytes,
            &mut s.tx_packets,
            &mut s.tx_errs,
            &mut s.tx_drop,
            &mut s.tx_fifo,
            &mut s.tx_colls,
            &mut s.tx_carrier,
            &mut s.tx_compressed,
        ];

        for (field_str, field_ref) in line.split_whitespace().zip(fields) {
            *field_ref = field_str.parse::<u64>()?;
        }
        Ok(s)
    }
}

#[derive(Debug, Clone)]
pub struct NetDevStats {
    pub interfaces: HashMap<String, InterfaceStats>,
}

impl NetDevStats {
    pub fn read() -> Result<NetDevStats, std::io::Error> {
        let data = read_to_string_with_limit(Path::new(PATH), 10 * 1024 * 1024)?;
        Ok(Self::parse(&data))
    }

    pub fn parse(data: &str) -> NetDevStats {
        let mut s = NetDevStats {
            interfaces: HashMap::new(),
        };

        let mut lines_iter = data.split('\n');
        // The first two lines contain a header
        lines_iter.next();
        lines_iter.next();
        for (_index, line) in lines_iter.enumerate() {
            if let Err(_e) = s.parse_line(line) {
                // We ignore errors parsing individual lines.
                // TODO: perhaps add a --verbose option to report warnings in the future, but in
                // general we don't want to spew lots of warnings on every stat update attempt.
                // eprintln!("{}:{} {:?}", PATH, _index + 3, _e);
            }
        }

        s
    }

    fn parse_line(&mut self, line: &str) -> Result<(), ParseError> {
        if line.is_empty() {
            // This happens after the final newline in the file.
            return Ok(());
        }

        let line = line.trim_start();
        let (if_name, line) = line.split_once(':').ok_or(ParseError::MissingField)?;
        self.interfaces
            .insert(if_name.to_string(), InterfaceStats::parse(line)?);
        Ok(())
    }
}

impl crate::stats::StatType for NetDevStats {
    fn name() -> &'static str {
        PATH
    }

    fn new_zero() -> Self {
        Self {
            interfaces: HashMap::new(),
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
        let s = NetDevStats::parse(TEST_DATA);

        assert_eq!(s.interfaces.len(), 5);

        let lo = &s.interfaces.get("lo").ok_or(anyhow!("missing lo"))?;
        assert_eq!(lo.rx_bytes, 9377566);
        assert_eq!(lo.rx_packets, 83111);
        assert_eq!(lo.rx_errs, 0);
        assert_eq!(lo.tx_bytes, 9377566);
        assert_eq!(lo.tx_packets, 83111);

        let wifi = &s.interfaces.get("wlp0s1").ok_or(anyhow!("missing wlp0s1"))?;
        assert_eq!(wifi.rx_bytes, 5045788342);
        assert_eq!(wifi.rx_packets, 5352370);
        assert_eq!(wifi.rx_errs, 9);
        assert_eq!(wifi.rx_drop, 8);
        assert_eq!(wifi.rx_fifo, 7);
        assert_eq!(wifi.rx_frame, 6);
        assert_eq!(wifi.rx_compressed, 5);
        assert_eq!(wifi.rx_multicast, 4);
        assert_eq!(wifi.tx_bytes, 210809056);
        assert_eq!(wifi.tx_packets, 1073720);
        assert_eq!(wifi.tx_errs, 11);
        assert_eq!(wifi.tx_drop, 12);
        assert_eq!(wifi.tx_fifo, 13);
        assert_eq!(wifi.tx_colls, 14);
        assert_eq!(wifi.tx_carrier, 15);
        assert_eq!(wifi.tx_compressed, 16);

        let veth = &s.interfaces.get("veth1000_aBcD").ok_or(anyhow!("missing veth"))?;
        assert_eq!(veth.rx_bytes, 219648629692);
        assert_eq!(veth.rx_packets, 6895126);
        assert_eq!(veth.tx_bytes, 330764094);
        assert_eq!(veth.tx_packets, 4126861);

        Ok(())
    }

    const TEST_DATA: &str = r#"
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 9377566   83111    0    0    0     0          0         0  9377566   83111    0    0    0     0       0          0
wlp0s1: 5045788342 5352370    9    8    7     6          5         4 210809056 1073720   11   12   13    14      15         16
lxcbr0: 219552097928 6895126    0    0    0     0          0       182 330664771 4126237    0    0    0     0       0          0
enx520123456789:       0       0    0    0    0     0          0         0        0       0    0    0    0     0       0          0
veth1000_aBcD: 219648629692 6895126    0    0    0     0          0         0 330764094 4126861    0    0    0     0       0          0
"#;
}
