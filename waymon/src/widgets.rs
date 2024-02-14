pub mod timeseries;
pub mod cpu;
pub mod disk_io;

pub trait Widget {
    fn update(&mut self);
}
