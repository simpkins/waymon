use crate::collectors::procstat::ProcStat;
use crate::config::CpuWidgetConfig;
use crate::stats::{AllStats, StatsDelta};
use crate::widgets::timeseries::TimeseriesChart;
use crate::widgets::Widget;
use crate::waymon::Waymon;
use gtk::cairo;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub struct CpuWidget {
    stats: Rc<RefCell<StatsDelta<ProcStat>>>,
    usage_ratio: f64,
}

impl CpuWidget {
    pub fn new(
        container: &gtk::Box,
        config: &CpuWidgetConfig,
        all_stats: &mut AllStats,
    ) -> Rc<RefCell<CpuWidget>> {
        let widget = Rc::new(RefCell::new(CpuWidget {
            stats: all_stats.get_proc_stats(),
            usage_ratio: 0.0,
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
        cr.set_source_rgb(1.0, 0.0, 0.0);
        cr.move_to(width as f64 * 0.5, height as f64 * 0.5);
        cr.line_to(width as f64 * 0.5, height as f64 * 0.75);
        let _ = cr.stroke();
    }
}

impl Widget for CpuWidget {
    fn update(&mut self) {
        let s = self.stats.borrow();
        let (new, old) = s.get_new_and_old();
        let user = new.cpu.user - old.cpu.user;
        let nice = new.cpu.nice - old.cpu.nice;
        let system = new.cpu.system - old.cpu.system;
        let idle = new.cpu.idle - old.cpu.idle;
        let total_used = user + system + nice;
        self.usage_ratio = total_used / (total_used + idle);
        eprintln!("CPU usage: {:.2}", self.usage_ratio * 100.0)
        // TODO: read stats
        // TODO: mark that the drawing area needs to be redrawn
    }
}
