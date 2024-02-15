use gtk::cairo;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub struct Chart {}

pub trait ChartDrawCallback {
    fn draw(&self, cr: &cairo::Context, width: i32, height: i32);
}

impl Chart {
    pub fn configure(
        da: &gtk::DrawingArea,
        height: u32,
        callback: Rc<RefCell<dyn ChartDrawCallback>>,
    ) {
        da.add_css_class("chart");
        da.set_content_height(height as i32);

        let weak_cb: Weak<RefCell<dyn ChartDrawCallback>> = Rc::downgrade(&callback);
        da.set_draw_func(move |_, cr, width, height| {
            if let Some(cb) = weak_cb.upgrade() {
                cb.borrow().draw(cr, width, height);
            }
        });
    }
}

pub struct TimeseriesChart {}

impl TimeseriesChart {
    pub fn new() -> Self {
        Self {
            // annotation: String::new(),
        }
    }

    pub fn add_ui<P: FnMut(&gtk::DrawingArea, &cairo::Context, i32, i32) + 'static>(
        &self,
        container: &gtk::Box,
        width: i32,
        height: u32,
        draw_func: P,
    ) {
        let da = gtk::DrawingArea::new();
        da.add_css_class("chart");
        da.set_content_width(width);
        da.set_content_height(height as i32);
        da.set_draw_func(draw_func);
        container.append(&da);
    }
}

pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Color {
    pub fn new(r: f64, g: f64, b: f64) -> Color {
        Color { r, g, b }
    }
}

pub struct StackedTimeseriesChart<const NUM_SERIES: usize> {
    data: Vec<[u64; NUM_SERIES]>,
    colors: [Color; NUM_SERIES],
    next_index: usize,
}

fn get_default_color(idx: usize, num_colors: usize) -> Color {
    let pct = (idx as f64) / (num_colors as f64);
    Color::new(0.0, 0.0, 1.0 - pct)
}

impl<const NUM_SERIES: usize> StackedTimeseriesChart<NUM_SERIES> {
    pub fn new(ts_size: usize) -> StackedTimeseriesChart<NUM_SERIES> {
        let mut chart = StackedTimeseriesChart::<NUM_SERIES> {
            data: Vec::with_capacity(ts_size),
            colors: core::array::from_fn(|idx| get_default_color(idx, NUM_SERIES)),
            next_index: 0,
        };
        chart.data.resize(ts_size, core::array::from_fn(|_| 0));
        chart
    }

    pub fn add_values(&mut self, v: [u64; NUM_SERIES]) {
        self.data[self.next_index] = v;
        self.next_index += 1;
        if self.next_index >= self.data.len() {
            self.next_index = 0;
        }
    }

    pub fn draw(&self, cr: &cairo::Context, width: i32, height: i32) {
        let x_scale: f64 = 1.0;
        let y_scale: f64 = 1.0;

        cr.set_line_width(1.0);
        let mut idx = self.next_index;
        let mut x: f64 = (width as f64) - (0.5 * x_scale);
        loop {
            if idx == 0 {
                idx = self.data.len() - 1;
            } else {
                idx -= 1;
            }
            if idx == self.next_index {
                break;
            }

            let entry = &self.data[idx];
            let mut cur_height = height as f64;
            for ts_idx in 0..NUM_SERIES {
                let value = entry[ts_idx];
                let c = &self.colors[ts_idx];
                cr.move_to(x, cur_height);
                cr.set_source_rgb(c.r, c.g, c.b);
                let y = (value as f64) * y_scale;
                cur_height -= y;
                cr.line_to(x, cur_height);
                let _ = cr.stroke();
                if cur_height <= 0.0 {
                    break;
                }
            }

            x -= x_scale;
            if x < 0.0 {
                break;
            }
        }
    }
}
