pub mod timeseries;

use crate::config::CpuWidgetConfig;
use crate::waymon::Waymon;
use gtk::cairo;
use std::rc::{Rc, Weak};

pub trait Widget {}

pub struct CpuWidget {}

impl CpuWidget {
    pub fn new(container: &gtk::Box, config: &CpuWidgetConfig) -> Rc<CpuWidget> {
        let widget = Rc::new(CpuWidget {});
        Waymon::add_widget_label(container, &config.label);
        let chart = timeseries::TimeseriesChart::new();

        let weak: Weak<Self> = Rc::downgrade(&widget);
        chart.add_ui(container, -1, config.height, move |_, cr, width, height| {
            if let Some(w) = weak.upgrade() {
                w.draw(cr, width, height);
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

impl Widget for CpuWidget {}

pub struct DiskIoWidget {}

impl DiskIoWidget {
    pub fn new() -> Self {
        Self {}
    }
}

impl Widget for DiskIoWidget {}
