use crate::collectors::diskstats::{ProcDiskStats, BYTES_PER_SECTOR};
use crate::config::default_chart_height;
use crate::stats::{AllStats, StatsDelta};
use crate::util::humanify_f64;
use crate::waymon::Waymon;
use crate::widgets::timeseries::{Chart, ChartDrawCallback, StackedTimeseriesChart};
use crate::widgets::Widget;
use gtk::cairo;
use gtk::prelude::*;
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use tracing::warn;

// We suppress the non_snake_case warning here so that we can more clearly disambiguate Bps (bytes
// per second) from bps (bits per second).
#[allow(non_snake_case)]
pub struct DiskIoWidget {
    disk: String,
    stats: Rc<RefCell<StatsDelta<ProcDiskStats>>>,
    container: gtk::Box,
    da: gtk::DrawingArea,
    chart: StackedTimeseriesChart<f64, 2>,
    disk_present: bool,
    busy_fraction: f64,
    read_Bps: f64,
    write_Bps: f64,
}

#[derive(Debug, Deserialize)]
pub struct DiskIoWidgetConfig {
    pub label: String,
    pub disk: String,

    #[serde(default = "default_chart_height")]
    pub height: u32,
}

impl DiskIoWidgetConfig {
    pub fn create_widget(
        &self,
        all_stats: &mut AllStats,
        history_length: usize,
    ) -> Rc<RefCell<DiskIoWidget>> {
        let widget_rc = Rc::new(RefCell::new(DiskIoWidget {
            disk: self.disk.clone(),
            stats: all_stats.get_disk_stats(),
            container: gtk::Box::new(gtk::Orientation::Vertical, /*spacing*/ 0),
            da: gtk::DrawingArea::new(),
            chart: StackedTimeseriesChart::new(history_length),
            // Initialize disk_present to true so that we will log a warning once
            // if it is actually not present.
            disk_present: true,
            busy_fraction: 0.0,
            read_Bps: 0.0,
            write_Bps: 0.0,
        }));
        {
            let widget = widget_rc.borrow();
            Waymon::add_widget_label(&widget.container, &self.label);
            Chart::configure(&widget.da, self.height, widget_rc.clone());
            widget.container.append(&widget.da);
        }
        widget_rc
    }
}

impl ChartDrawCallback for DiskIoWidget {
    fn draw(&self, cr: &cairo::Context, width: i32, height: i32) {
        let max_value = self.chart.max_value();
        let y_scale = if max_value <= 0.0 {
            1.0
        } else {
            ((height - 2) as f64) / max_value
        };
        self.chart.draw(cr, width, height, y_scale);

        if self.disk_present {
            let annotation = format!(
                "{}/s R\n{}/s W\n{:.0}% busy",
                humanify_f64(self.read_Bps, 3),
                humanify_f64(self.write_Bps, 3),
                self.busy_fraction * 100.0
            );
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

            let sectors_read = new.num_sectors_read - old.num_sectors_read;
            let sectors_written = new.num_sectors_written - old.num_sectors_written;
            let read_bytes = sectors_read * BYTES_PER_SECTOR;
            let write_bytes = sectors_written * BYTES_PER_SECTOR;
            self.read_Bps = (read_bytes as f64) / delta_secs;
            self.write_Bps = (write_bytes as f64) / delta_secs;
            self.chart.add_values(&[self.read_Bps, self.write_Bps])
        } else if self.disk_present {
            warn!("disk {} not present", &self.disk);
            self.disk_present = false;
            self.chart.add_values(&[0.0, 0.0]);
        }

        // Mark that the drawing area needs to be redrawn
        self.da.queue_draw();
    }

    fn gtk_widget<'a>(&'a self) -> &'a gtk::Box {
        &self.container
    }
}
