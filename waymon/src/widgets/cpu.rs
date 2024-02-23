use crate::collectors::procstat::ProcStat;
use crate::config::CpuWidgetConfig;
use crate::stats::{AllStats, StatsDelta};
use crate::waymon::Waymon;
use crate::widgets::timeseries::{Chart, ChartDrawCallback, StackedTimeseriesChart};
use crate::widgets::Widget;
use gtk::cairo;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct CpuWidget {
    stats: Rc<RefCell<StatsDelta<ProcStat>>>,
    container: gtk::Box,
    da: gtk::DrawingArea,
    chart: StackedTimeseriesChart<f64, 3>,
    usage_ratio: f64,
}

impl CpuWidget {
    pub fn new(
        config: &CpuWidgetConfig,
        all_stats: &mut AllStats,
        history_length: usize,
    ) -> Rc<RefCell<CpuWidget>> {
        let widget_rc = Rc::new(RefCell::new(CpuWidget {
            stats: all_stats.get_proc_stats(),
            container: gtk::Box::new(gtk::Orientation::Vertical, /*spacing*/ 0),
            da: gtk::DrawingArea::new(),
            chart: StackedTimeseriesChart::new(history_length),
            usage_ratio: 0.0,
        }));
        {
            let widget = widget_rc.borrow();
            Waymon::add_widget_label(&widget.container, &config.label);
            Chart::configure(&widget.da, config.height, widget_rc.clone());
            widget.container.append(&widget.da);
        }
        widget_rc
    }
}

impl ChartDrawCallback for CpuWidget {
    fn draw(&self, cr: &cairo::Context, width: i32, height: i32) {
        let max_value = self.chart.max_value();
        let y_max = if max_value < 5.0 {
            5.0
        } else if max_value < 10.0 {
            10.0
        } else if max_value < 25.0 {
            25.0
        } else if max_value < 50.0 {
            50.0
        } else {
            100.0
        };

        let y_scale = (height as f64) / y_max;
        self.chart.draw(cr, width, height, y_scale);
        Chart::draw_annotation(
            &self.da,
            cr,
            width,
            height,
            &format!("{}%", (self.usage_ratio * 100.0) as u32),
        );
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
        let total = total_used + idle;
        self.usage_ratio = total_used / total;

        let total_f64 = total.value() as f64;
        let nice_pct = 100.0 * (nice.value() as f64) / total_f64;
        let user_pct = 100.0 * (user.value() as f64) / total_f64;
        let system_pct = 100.0 * (system.value() as f64) / total_f64;

        self.chart.add_values(&[nice_pct, user_pct, system_pct]);

        // Mark that the drawing area needs to be redrawn
        self.da.queue_draw();
    }

    fn gtk_widget<'a>(&'a self) -> &'a gtk::Box {
        &self.container
    }
}
