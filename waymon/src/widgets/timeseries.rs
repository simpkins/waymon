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
        // We accept callback by value here rather than by reference, since we expect most callers
        // to have an Rc<RefCell<SomeConcreteType>>, and they need to clone the Rc anyway to
        // convert from an RefCell<SomeConcreteType> to a RefCell<dyn ChartDrawCallback>.
        // Accepting the argument by value makes this syntax simpler for callers, since Rust will
        // automatically figure out the type conversion for them when they clone.
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
        /*
        let weak_cb: Weak<RefCell<dyn ChartDrawCallback>> = Rc::downgrade(&callback);
        da.connect_root_notify(move |_| {
            if let Some(cb) = weak_cb.upgrade() {
                cb.borrow().root_changed();
            }
        });
        */
    }

    pub fn draw_annotation(
        da: &gtk::DrawingArea,
        cr: &cairo::Context,
        _width: i32,
        _height: i32,
        text: &str,
    ) {
        // TODO: store the layout as a member in the timeseries chart, rather than recreating it
        // each time.  According to the pango cairo docs we need to regenerate the layout whenever
        // the widget root changes.  However, in our case we never change the widget root.

        let layout = da.create_pango_layout(Some(text));
        /*
        let pango_ctx = gtk::pango::Context::new();
        pango_ctx.set_font_map(da.pango_context().font_map().as_ref());
        let layout = gtk::pango::Layout::new(&pango_ctx);
        layout.set_text(text);
        */

        let font_desc = gtk::pango::FontDescription::from_string("Sans 9");
        layout.set_font_description(Some(&font_desc));

        // pango_layout_set_alignment(layout, PANGO_ALIGN_RIGHT);
        // pango_layout_set_font_description(layout, font_desc_);

        cr.set_source_rgb(0.4, 0.4, 0.4);
        cr.move_to(2.0, 0.0);
        pangocairo::functions::show_layout(cr, &layout);
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
