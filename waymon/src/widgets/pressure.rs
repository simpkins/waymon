use crate::collectors::pressure::{CpuPressure, IoPressure, MemoryPressure, PressureStats};
use crate::config::default_chart_height;
use crate::stats::{AllStats, StatType, StatsDelta};
use crate::waymon::Waymon;
use crate::widgets::timeseries::{Chart, ChartDrawCallback, StackedTimeseriesChart};
use crate::widgets::Widget;
use gtk::cairo;
use gtk::prelude::*;
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub struct PressureWidget<T: PressureStats + StatType + 'static> {
    stats: Rc<RefCell<StatsDelta<T>>>,
    container: gtk::Box,
    da: gtk::DrawingArea,
    chart: StackedTimeseriesChart<f64, 2>,
    some_fraction: f64,
    full_fraction: f64,
}

impl<T: PressureStats + StatType + 'static> PressureWidget<T> {
    pub fn new(
        stats: Rc<RefCell<StatsDelta<T>>>,
        label: &str,
        history_length: usize,
        height: u32,
    ) -> Rc<RefCell<PressureWidget<T>>> {
        let widget_rc = Rc::new(RefCell::new(PressureWidget::<T> {
            stats,
            container: gtk::Box::new(gtk::Orientation::Vertical, /*spacing*/ 0),
            da: gtk::DrawingArea::new(),
            chart: StackedTimeseriesChart::new(history_length),
            some_fraction: 0.0,
            full_fraction: 0.0,
        }));
        {
            let widget = widget_rc.borrow();
            Waymon::add_widget_label(&widget.container, label);
            Chart::configure(&widget.da, height, widget_rc.clone());
            widget.container.append(&widget.da);
        }
        widget_rc
    }
}

#[derive(Debug, Deserialize)]
pub struct CpuPressureWidgetConfig {
    pub label: String,
    #[serde(default = "default_chart_height")]
    pub height: u32,
}

impl CpuPressureWidgetConfig {
    pub fn create_widget(
        &self,
        all_stats: &mut AllStats,
        history_length: usize,
    ) -> Rc<RefCell<PressureWidget<CpuPressure>>> {
        PressureWidget::<CpuPressure>::new(
            all_stats.get_cpu_pressure(),
            &self.label,
            history_length,
            self.height,
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct IoPressureWidgetConfig {
    pub label: String,
    #[serde(default = "default_chart_height")]
    pub height: u32,
}

impl IoPressureWidgetConfig {
    pub fn create_widget(
        &self,
        all_stats: &mut AllStats,
        history_length: usize,
    ) -> Rc<RefCell<PressureWidget<IoPressure>>> {
        PressureWidget::<IoPressure>::new(
            all_stats.get_io_pressure(),
            &self.label,
            history_length,
            self.height,
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct MemPressureWidgetConfig {
    pub label: String,
    #[serde(default = "default_chart_height")]
    pub height: u32,
}

impl MemPressureWidgetConfig {
    pub fn create_widget(
        &self,
        all_stats: &mut AllStats,
        history_length: usize,
    ) -> Rc<RefCell<PressureWidget<MemoryPressure>>> {
        PressureWidget::<MemoryPressure>::new(
            all_stats.get_mem_pressure(),
            &self.label,
            history_length,
            self.height,
        )
    }
}

impl<T: PressureStats + StatType> ChartDrawCallback for PressureWidget<T> {
    fn draw(&self, cr: &cairo::Context, width: i32, height: i32) {
        let max_value = self.chart.max_value();
        let y_scale = if max_value <= 0.0 {
            1.0
        } else {
            ((height - 2) as f64) / max_value
        };
        self.chart.draw(cr, width, height, y_scale);

        let annotation = format!(
            "some: {:.0}%\nfull: {:.0}%\n",
            self.some_fraction * 100.0,
            self.full_fraction * 100.0,
        );
        Chart::draw_annotation(&self.da, cr, width, height, &annotation);
    }
}

impl<T: PressureStats + StatType> Widget for PressureWidget<T> {
    fn update(&mut self) {
        let s = self.stats.borrow();
        let (new, old) = s.get_new_and_old();

        let some = Duration::from_micros((new.some_us() - old.some_us()) as u64);
        let full = Duration::from_micros((new.full_us() - old.full_us()) as u64);
        let delta_secs = s.time_delta().as_secs_f64();

        // According to the documentation, it seems like the "some" count should include the "full"
        // count: some tasks are always blocked whenever all tasks are blocked.  However, the
        // accounting doesn't appear to be exact: the "full" count is sometimes 1us higher than the
        // "some" count.  Therefore use saturating_sub() here.
        let some_exclusive = some.saturating_sub(full);

        self.some_fraction = some.as_secs_f64() / delta_secs;
        self.full_fraction = full.as_secs_f64() / delta_secs;

        self.chart
            .add_values(&[some_exclusive.as_secs_f64(), full.as_secs_f64()]);

        // Mark that the drawing area needs to be redrawn
        self.da.queue_draw();
    }

    fn gtk_widget<'a>(&'a self) -> &'a gtk::Box {
        &self.container
    }
}
