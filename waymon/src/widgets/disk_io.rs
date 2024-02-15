use crate::collectors::diskstats::{ProcDiskStats, BYTES_PER_SECTOR};
use crate::config::DiskIoWidgetConfig;
use crate::stats::{AllStats, StatsDelta};
use crate::waymon::Waymon;
use crate::widgets::timeseries::{Chart, ChartDrawCallback, StackedTimeseriesChart};
use crate::widgets::Widget;
use gtk::cairo;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

// We suppress the non_snake_case warning here so that we can more clearly disambiguate Bps (bytes
// per second) from bps (bits per second).
#[allow(non_snake_case)]
pub struct DiskIoWidget {
    disk: String,
    stats: Rc<RefCell<StatsDelta<ProcDiskStats>>>,
    da: gtk::DrawingArea,
    chart: StackedTimeseriesChart<2>,
    disk_present: bool,
    busy_fraction: f64,
    read_Bps: f64,
    write_Bps: f64,
}

impl DiskIoWidget {
    pub fn new(
        container: &gtk::Box,
        config: &DiskIoWidgetConfig,
        all_stats: &mut AllStats,
        history_length: usize,
    ) -> Rc<RefCell<DiskIoWidget>> {
        let widget_rc = Rc::new(RefCell::new(DiskIoWidget {
            disk: config.disk.clone(),
            stats: all_stats.get_disk_stats(),
            da: gtk::DrawingArea::new(),
            chart: StackedTimeseriesChart::<2>::new(history_length),
            // Initialize disk_present to true so that we will log a warning once
            // if it is actually not present.
            disk_present: true,
            busy_fraction: 0.0,
            read_Bps: 0.0,
            write_Bps: 0.0,
        }));
        {
            let widget = widget_rc.borrow();
            Waymon::add_widget_label(container, &config.label);
            Chart::configure(&widget.da, config.height, widget_rc.clone());
            container.append(&widget.da);
        }
        widget_rc
    }
}

fn humanify_f64(value: f64) -> String {
  if value < 1000.0 {
    return format!("{}B", value);
  }
  if value < 1_000_000.0 {
    return format!("{}KB", (value as u64) / 1000);
  }
  if value < 1_000_000_000.0 {
    return format!("{}MB", (value as u64) / 1_000_000);
  }
  if value < 1_000_000_000_000.0 {
    return format!("{}GB", (value as u64) / 1_000_000_000);
  }
  return format!("{}TB", (value as u64) / 1_000_000_000_000);
}

impl ChartDrawCallback for DiskIoWidget {
    fn draw(&self, cr: &cairo::Context, width: i32, height: i32) {
        self.chart.draw(cr, width, height);

        if self.disk_present {
            let annotation = format!(
                "{}/s R\n{}/s W\n{:.0}% busy",
                humanify_f64(self.read_Bps),
                humanify_f64(self.write_Bps),
                0.0
            ); // self.busy_fraction * 100.0);
            Chart::draw_annotation(&self.da, cr, width, height, &annotation);
        } else {
            Chart::draw_annotation(&self.da, cr, width, height, "Not Present");
        }
    }
}

impl Widget for DiskIoWidget {
    fn update(&mut self) {
        let s = self.stats.borrow();
        let (new_stats, old_stats) = s.get_new_and_old();
        if let (Some(new), Some(old)) = (
            new_stats.disks.get(&self.disk),
            old_stats.disks.get(&self.disk),
        ) {
            self.disk_present = true;
            let ms_busy = Duration::from_millis((new.ms_doing_io - old.ms_doing_io) as u64);
            let delta_secs = s.time_delta().as_secs_f64();
            self.busy_fraction = ms_busy.as_secs_f64() / delta_secs;
            eprintln!(
                "{} disk usage: {:?} busy, {:?} total",
                &self.disk,
                ms_busy,
                s.time_delta()
            );

            let sectors_read = new.num_sectors_read - old.num_sectors_read;
            let sectors_written = new.num_sectors_written - old.num_sectors_written;
            let read_bytes = sectors_read * BYTES_PER_SECTOR;
            let write_bytes = sectors_written * BYTES_PER_SECTOR;
            self.read_Bps = (read_bytes as f64) / delta_secs;
            self.write_Bps = (write_bytes as f64) / delta_secs;
            self.chart.add_values([read_bytes, write_bytes])
        } else if self.disk_present {
            eprintln!("{} disk not present", &self.disk);
            self.disk_present = false;
            self.chart.add_values([0, 0]);
        }

        // Mark that the drawing area needs to be redrawn
        self.da.queue_draw();
    }
}