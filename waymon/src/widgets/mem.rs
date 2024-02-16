use crate::collectors::meminfo::MemoryStats;
use crate::config::MemWidgetConfig;
use crate::stats::{AllStats, StatsDelta};
use crate::util::humanify_f64;
use crate::waymon::Waymon;
use crate::widgets::timeseries::{Chart, ChartDrawCallback, StackedTimeseriesChart};
use crate::widgets::Widget;
use gtk::cairo;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct MemWidget {
    stats: Rc<RefCell<StatsDelta<MemoryStats>>>,
    da: gtk::DrawingArea,
    chart: StackedTimeseriesChart<u64, 2>,
    mem_available_kb: u64,
    mem_total_kb: u64,
}

impl MemWidget {
    pub fn new(
        container: &gtk::Box,
        config: &MemWidgetConfig,
        all_stats: &mut AllStats,
        history_length: usize,
    ) -> Rc<RefCell<MemWidget>> {
        let widget_rc = Rc::new(RefCell::new(MemWidget {
            stats: all_stats.get_mem_stats(),
            da: gtk::DrawingArea::new(),
            chart: StackedTimeseriesChart::new(history_length),
            mem_available_kb: 0,
            mem_total_kb: 0,
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

impl ChartDrawCallback for MemWidget {
    fn draw(&self, cr: &cairo::Context, width: i32, height: i32) {
        let max_value = std::cmp::max(self.mem_total_kb, 1024);
        let y_scale = (height as f64) / (max_value as f64);
        self.chart.draw(cr, width, height, y_scale);

        let used_kb = self.mem_total_kb - self.mem_available_kb;
        let pct_used = 100.0 * ((used_kb as f64) / (self.mem_total_kb as f64));
        let annotation = format!(
            "{:.0}% used\n {}/{}",
            pct_used,
            humanify_f64((used_kb * 1024) as f64, 2),
            humanify_f64((self.mem_total_kb * 1024) as f64, 2),
        );
        Chart::draw_annotation(&self.da, cr, width, height, &annotation);
    }
}

impl Widget for MemWidget {
    fn update(&mut self) {
        let s = self.stats.borrow();
        let new_stats = s.get_new();

        // Memory stats are a little complicated to convey accurately: it's often the case that
        // relatively little memory is actually unused ("free").  However, there may be plenty of
        // memory that is used being for filesystem caches, temporary buffers, etc which the kernel
        // could reclaim on-demand if needed.  The mem_available stat reports how much the kernel
        // thinks it could reclaim immediately if needed, and includes the amount reported in
        // mem_free.  Various other fields in the MemoryStats break down some of the other ways
        // this memory is being used, but we don't bother reporting any of that here.
        self.mem_total_kb = new_stats.mem_total;
        self.mem_available_kb = new_stats.mem_available;

        // We graph "unavailable" memory at the bottom of the chart, then "used but available"
        // memory, and leave "free" uncolored at the top of the chart.
        let unavailable_kb = new_stats.mem_total - new_stats.mem_available;
        let available_used_kb = new_stats.mem_available - new_stats.mem_free;
        self.chart.add_values(&[unavailable_kb, available_used_kb]);

        // Mark that the drawing area needs to be redrawn
        self.da.queue_draw();
    }
}
