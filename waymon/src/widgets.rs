pub mod cpu;
pub mod disk_io;
pub mod mem;
pub mod net;
pub mod timeseries;

pub trait Widget {
    fn update(&mut self);
}
