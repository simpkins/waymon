use crate::read::read_to_string_with_limit;
use crate::stats::StatsError;
use std::path::Path;

const CPU_PATH: &str = "/proc/pressure/cpu";
const IO_PATH: &str = "/proc/pressure/io";
const MEMORY_PATH: &str = "/proc/pressure/memory";

#[derive(Debug, Clone, Default)]
pub struct CpuPressure {
    pub some: u64,
    pub full: u64,
}

impl CpuPressure {
    pub fn read() -> Result<Self, StatsError> {
        Self::parse(&read_pressure_file(CPU_PATH)?)
    }

    pub fn parse(data: &str) -> Result<Self, StatsError> {
        let (some, full) = parse_pressure_data(data)?;
        Ok(Self { some, full })
    }
}

impl crate::stats::StatType for CpuPressure {
    fn name() -> &'static str {
        CPU_PATH
    }

    fn new_zero() -> Self {
        Default::default()
    }

    fn update(&mut self) -> Result<(), crate::stats::StatsError> {
        *self = Self::read()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct IoPressure {
    pub some: u64,
    pub full: u64,
}

impl IoPressure {
    pub fn read() -> Result<Self, StatsError> {
        Self::parse(&read_pressure_file(IO_PATH)?)
    }

    pub fn parse(data: &str) -> Result<Self, StatsError> {
        let (some, full) = parse_pressure_data(data)?;
        Ok(Self { some, full })
    }
}

impl crate::stats::StatType for IoPressure {
    fn name() -> &'static str {
        IO_PATH
    }

    fn new_zero() -> Self {
        Default::default()
    }

    fn update(&mut self) -> Result<(), crate::stats::StatsError> {
        *self = Self::read()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryPressure {
    pub some: u64,
    pub full: u64,
}

impl MemoryPressure {
    pub fn read() -> Result<Self, StatsError> {
        Self::parse(&read_pressure_file(MEMORY_PATH)?)
    }

    pub fn parse(data: &str) -> Result<MemoryPressure, StatsError> {
        let (some, full) = parse_pressure_data(data)?;
        Ok(MemoryPressure { some, full })
    }
}

impl crate::stats::StatType for MemoryPressure {
    fn name() -> &'static str {
        MEMORY_PATH
    }

    fn new_zero() -> Self {
        Default::default()
    }

    fn update(&mut self) -> Result<(), crate::stats::StatsError> {
        *self = Self::read()?;
        Ok(())
    }
}

fn read_pressure_file(path_str: &str) -> Result<String, std::io::Error> {
    read_to_string_with_limit(Path::new(path_str), 4096)
}

fn parse_pressure_data(data: &str) -> Result<(u64, u64), StatsError> {
    let mut some: u64 = 0;
    let mut full: u64 = 0;
    for (_index, line) in data.split('\n').enumerate() {
        if let Some(data) = line.strip_prefix("some ") {
            some = parse_pressure_line(data)?;
        } else if let Some(data) = line.strip_prefix("full ") {
            full = parse_pressure_line(data)?;
        }
    }

    Ok((some, full))
}

fn parse_pressure_line(data: &str) -> Result<u64, StatsError> {
    const TOTAL_PREFIX: &str = "total=";
    if let Some(start) = data.find(TOTAL_PREFIX) {
        let data = &data[start + TOTAL_PREFIX.len()..];
        return data
            .parse::<u64>()
            .map_err(|_| StatsError::ParseError("invalid integer in Linux PSI file".to_string()));
    }
    Err(StatsError::ParseError(format!(
        "unparseable Linux PSI line: {:?}",
        data
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_parse() -> Result<()> {
        let s = MemoryPressure::parse(TEST_DATA)?;

        assert_eq!(s.some, 2062279);
        assert_eq!(s.full, 1895827);

        Ok(())
    }

    const TEST_DATA: &str = r#"
some avg10=0.00 avg60=0.08 avg300=0.42 total=2062279
full avg10=0.00 avg60=0.00 avg300=0.00 total=1895827
"#;
}
