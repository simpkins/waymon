use crate::collectors::diskstats::ProcDiskStats;
use crate::collectors::meminfo::MemoryStats;
use crate::collectors::net::NetDevStats;
use crate::collectors::pressure::{CpuPressure, IoPressure, MemoryPressure};
use crate::collectors::procstat::ProcStat;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StatsError {
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("parse error: {0}")]
    ParseError(String),
}

pub trait StatType: Clone {
    fn new_zero() -> Self;
    fn update(&mut self) -> Result<(), StatsError>;
    fn name() -> &'static str;
}

pub struct StatsDelta<T: StatType> {
    a: T,
    b: T,
    a_newer: bool,
    timestamp: Instant,
    duration: Duration,
}

impl<T: StatType> StatsDelta<T> {
    pub fn time_delta(&self) -> Duration {
        self.duration
    }

    pub fn get_new_and_old(&self) -> (&T, &T) {
        if self.a_newer {
            (&self.a, &self.b)
        } else {
            (&self.b, &self.a)
        }
    }

    pub fn get_new(&self) -> &T {
        if self.a_newer {
            &self.a
        } else {
            &self.b
        }
    }
}

pub trait StatsDeltaConstructor {
    fn new() -> Self;
}

pub trait StatsDeltaIntf {
    fn name(&self) -> &'static str;
    fn update(&mut self, now: Instant) -> Result<(), StatsError>;
}

impl<T: StatType> StatsDeltaConstructor for StatsDelta<T> {
    fn new() -> Self {
        let now = Instant::now();
        let mut s = T::new_zero();
        if let Err(e) = s.update() {
            eprintln!("error initializing {} stats: {:?}", T::name(), e);
            // Fall through anyway and initialize the structure with 0 values
        }

        Self {
            a: s.clone(),
            b: s.clone(),
            a_newer: true,
            timestamp: now,
            duration: Duration::from_millis(0),
        }
    }
}

impl<T: StatType> StatsDeltaIntf for StatsDelta<T> {
    fn name(&self) -> &'static str {
        T::name()
    }

    fn update(&mut self, now: Instant) -> Result<(), StatsError> {
        if self.a_newer {
            self.b.update()?;
        } else {
            self.a.update()?;
        }
        self.a_newer = !self.a_newer;
        self.duration = now - self.timestamp;
        self.timestamp = now;
        Ok(())
    }
}

#[derive(Default)]
pub struct AllStats {
    proc_stats: Option<Rc<RefCell<StatsDelta<ProcStat>>>>,
    disk_stats: Option<Rc<RefCell<StatsDelta<ProcDiskStats>>>>,
    net_stats: Option<Rc<RefCell<StatsDelta<NetDevStats>>>>,
    mem_stats: Option<Rc<RefCell<StatsDelta<MemoryStats>>>>,
    cpu_pressure: Option<Rc<RefCell<StatsDelta<CpuPressure>>>>,
    io_pressure: Option<Rc<RefCell<StatsDelta<IoPressure>>>>,
    mem_pressure: Option<Rc<RefCell<StatsDelta<MemoryPressure>>>>,
}

impl AllStats {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_proc_stats(&mut self) -> Rc<RefCell<StatsDelta<ProcStat>>> {
        Self::get_stat(&mut self.proc_stats)
    }

    pub fn get_disk_stats(&mut self) -> Rc<RefCell<StatsDelta<ProcDiskStats>>> {
        Self::get_stat(&mut self.disk_stats)
    }

    pub fn get_net_stats(&mut self) -> Rc<RefCell<StatsDelta<NetDevStats>>> {
        Self::get_stat(&mut self.net_stats)
    }

    pub fn get_mem_stats(&mut self) -> Rc<RefCell<StatsDelta<MemoryStats>>> {
        Self::get_stat(&mut self.mem_stats)
    }

    pub fn get_cpu_pressure(&mut self) -> Rc<RefCell<StatsDelta<CpuPressure>>> {
        Self::get_stat(&mut self.cpu_pressure)
    }

    pub fn get_io_pressure(&mut self) -> Rc<RefCell<StatsDelta<IoPressure>>> {
        Self::get_stat(&mut self.io_pressure)
    }

    pub fn get_mem_pressure(&mut self) -> Rc<RefCell<StatsDelta<MemoryPressure>>> {
        Self::get_stat(&mut self.mem_pressure)
    }

    pub fn update(&mut self, now: Instant) {
        Self::update_stat(&mut self.proc_stats, now);
        Self::update_stat(&mut self.disk_stats, now);
        Self::update_stat(&mut self.net_stats, now);
        Self::update_stat(&mut self.mem_stats, now);
    }

    fn get_stat<T: StatType>(
        stat: &mut Option<Rc<RefCell<StatsDelta<T>>>>,
    ) -> Rc<RefCell<StatsDelta<T>>> {
        stat.get_or_insert_with(|| Rc::new(RefCell::new(StatsDelta::<T>::new())))
            .clone()
    }

    fn update_stat<T: StatType>(stat: &mut Option<Rc<RefCell<StatsDelta<T>>>>, now: Instant) {
        if let Some(stat_cell) = stat {
            match stat_cell.try_borrow_mut() {
                Ok(mut s) => {
                    if let Err(e) = s.update(now) {
                        eprintln!("error updating {}: {:?}", s.name(), e);
                    }
                }
                Err(_) => {
                    // This should only happen if we have a bug somewhere
                    eprintln!("error updating stats struct: stats data is currently borrowed");
                }
            }
        }
    }
}
