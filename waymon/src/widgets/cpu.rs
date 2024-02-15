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
    da: gtk::DrawingArea,
    chart: StackedTimeseriesChart<3>,
    usage_ratio: f64,
}

impl CpuWidget {
    pub fn new(
        container: &gtk::Box,
        config: &CpuWidgetConfig,
        all_stats: &mut AllStats,
        history_length: usize,
    ) -> Rc<RefCell<CpuWidget>> {
        let widget_rc = Rc::new(RefCell::new(CpuWidget {
            stats: all_stats.get_proc_stats(),
            da: gtk::DrawingArea::new(),
            chart: StackedTimeseriesChart::<3>::new(history_length),
            usage_ratio: 0.0,
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

impl ChartDrawCallback for CpuWidget {
    fn draw(&self, cr: &cairo::Context, width: i32, height: i32) {
        self.chart.draw(cr, width, height);
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
        eprintln!("CPU usage: {:.2}", self.usage_ratio * 100.0);

        self.chart
            .add_values([nice.value(), user.value(), system.value()]);

        // Mark that the drawing area needs to be redrawn
        self.da.queue_draw();
    }
}
