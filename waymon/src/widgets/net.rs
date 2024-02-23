use crate::collectors::net::NetDevStats;
use crate::config::NetWidgetConfig;
use crate::stats::{AllStats, StatsDelta};
use crate::util::humanify_f64;
use crate::waymon::Waymon;
use crate::widgets::timeseries::{Chart, ChartDrawCallback, StackedTimeseriesChart};
use crate::widgets::Widget;
use gtk::cairo;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::warn;

// We suppress the non_snake_case warning here so that we can more clearly disambiguate Bps (bytes
// per second) from bps (bits per second).
#[allow(non_snake_case)]
pub struct NetWidget {
    dev: String,
    stats: Rc<RefCell<StatsDelta<NetDevStats>>>,
    da: gtk::DrawingArea,
    chart: StackedTimeseriesChart<f64, 2>,
    dev_present: bool,
    rx_Bps: f64,
    tx_Bps: f64,
}

impl NetWidget {
    pub fn new(
        container: &gtk::Box,
        config: &NetWidgetConfig,
        all_stats: &mut AllStats,
        history_length: usize,
    ) -> Rc<RefCell<NetWidget>> {
        let widget_rc = Rc::new(RefCell::new(NetWidget {
            dev: config.dev.clone(),
            stats: all_stats.get_net_stats(),
            da: gtk::DrawingArea::new(),
            chart: StackedTimeseriesChart::new(history_length),
            dev_present: true,
            rx_Bps: 0.0,
            tx_Bps: 0.0,
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

impl ChartDrawCallback for NetWidget {
    fn draw(&self, cr: &cairo::Context, width: i32, height: i32) {
        let max_value = self.chart.max_value();
        let y_scale = if max_value <= 0.0 {
            1.0
        } else {
            ((height - 2) as f64) / max_value
        };
        self.chart.draw(cr, width, height, y_scale);

        if self.dev_present {
            let annotation = format!(
                "{}/s rx\n{}/s tx",
                humanify_f64(self.rx_Bps, 3),
                humanify_f64(self.tx_Bps, 3)
            );
            Chart::draw_annotation(&self.da, cr, width, height, &annotation);
        } else {
            Chart::draw_annotation(&self.da, cr, width, height, "Not Present");
        }
    }
}

impl Widget for NetWidget {
    fn update(&mut self) {
        let s = self.stats.borrow();
        let (new_stats, old_stats) = s.get_new_and_old();
        if let (Some(new), Some(old)) = (
            new_stats.interfaces.get(&self.dev),
            old_stats.interfaces.get(&self.dev),
        ) {
            self.dev_present = true;
            let delta_secs = s.time_delta().as_secs_f64();
            let rx_bytes = new.rx_bytes - old.rx_bytes;
            let tx_bytes = new.tx_bytes - old.tx_bytes;
            self.rx_Bps = (rx_bytes as f64) / delta_secs;
            self.tx_Bps = (tx_bytes as f64) / delta_secs;
            self.chart.add_values(&[self.rx_Bps, self.tx_Bps])
        } else if self.dev_present {
            warn!("interface {} not present", &self.dev);
            self.dev_present = false;
            self.chart.add_values(&[0.0, 0.0]);
        }

        // Mark that the drawing area needs to be redrawn
        self.da.queue_draw();
    }
}
