use crate::collectors::diskstats::ProcDiskStats;
use crate::config::DiskIoWidgetConfig;
use crate::stats::{AllStats, StatsDelta};
use crate::waymon::Waymon;
use crate::widgets::timeseries::TimeseriesChart;
use crate::widgets::Widget;
use gtk::cairo;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub struct DiskIoWidget {
    disk: String,
    stats: Rc<RefCell<StatsDelta<ProcDiskStats>>>,
    disk_present: bool,
}

impl DiskIoWidget {
    pub fn new(
        container: &gtk::Box,
        config: &DiskIoWidgetConfig,
        all_stats: &mut AllStats,
    ) -> Rc<RefCell<DiskIoWidget>> {
        let widget = Rc::new(RefCell::new(DiskIoWidget {
            disk: config.disk.clone(),
            stats: all_stats.get_disk_stats(),
            // Initialize disk_present to true so that we will log a warning once
            // if it is actually not present.
            disk_present: true,
        }));
        Waymon::add_widget_label(container, &config.label);
        let chart = TimeseriesChart::new();

        let weak: Weak<RefCell<Self>> = Rc::downgrade(&widget);
        chart.add_ui(container, -1, config.height, move |_, cr, width, height| {
            if let Some(w) = weak.upgrade() {
                w.borrow().draw(cr, width, height);
            }
        });
        widget
    }

    fn draw(&self, cr: &cairo::Context, width: i32, height: i32) {
        eprintln!("draw! w={width} h={height}");
        cr.set_line_width(1.0);
        cr.set_source_rgb(0.0, 0.0, 1.0);
        cr.move_to(width as f64 * 0.5, height as f64 * 0.5);
        cr.line_to(width as f64 * 0.75, height as f64 * 0.75);
        let _ = cr.stroke();
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
            let ms_busy = new.ms_doing_io - old.ms_doing_io;
            eprintln!(
                "{} disk usage: {:?} busy, {:?} total",
                &self.disk,
                ms_busy,
                s.time_delta()
            );
            self.disk_present = true;
        } else if self.disk_present {
            eprintln!("{} disk not present", &self.disk);
            self.disk_present = false;
        }
        // TODO: read stats
        // TODO: mark that the drawing area needs to be redrawn
    }
}
