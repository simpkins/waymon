use gtk::cairo;
use gtk::prelude::*;
use std::cell::RefCell;
use std::iter::Sum;
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

// We unfortunately can't use the standard Into() trait, since it isn't implemented for i64 and u64
// since f64 cannot represent the full 64-bit integer range.  In practice we don't really care
// about this issue for the extreme ends of the integer space.  If we encounter a situation where
// it does matter we could replace into_f64_lossy() with some sort of other API that takes the
// desired scaling factor as a parameter, since we know the scaled result should typically be a
// sane value between 0 and the height of the window in pixels.
pub trait IntoF64Lossy {
    fn into_f64_lossy(&self) -> f64;
}
impl IntoF64Lossy for f64 {
    fn into_f64_lossy(&self) -> f64 {
        *self
    }
}
impl IntoF64Lossy for u64 {
    fn into_f64_lossy(&self) -> f64 {
        *self as f64
    }
}

pub struct StackedTimeseriesChart<T, const NUM_SERIES: usize>
where
    T: Copy + Default + PartialOrd + Sum + IntoF64Lossy,
{
    data: Vec<[T; NUM_SERIES]>,
    colors: [Color; NUM_SERIES],
    next_index: usize,
    max_value: T,
}

fn get_default_color(idx: usize, num_colors: usize) -> Color {
    let pct = (idx as f64) / (num_colors as f64);
    Color::new(0.0, 0.0, 1.0 - pct)
}

impl<T, const NUM_SERIES: usize> StackedTimeseriesChart<T, NUM_SERIES>
where
    T: Copy + Default + PartialOrd + Sum + IntoF64Lossy,
{
    pub fn new(ts_size: usize) -> StackedTimeseriesChart<T, NUM_SERIES> {
        let mut chart = StackedTimeseriesChart::<T, NUM_SERIES> {
            data: Vec::with_capacity(ts_size),
            colors: core::array::from_fn(|idx| get_default_color(idx, NUM_SERIES)),
            next_index: 0,
            max_value: Default::default(),
        };
        chart
            .data
            .resize(ts_size, core::array::from_fn(|_| Default::default()));
        chart
    }

    // Returns the maximum total value stored in the timeseries.
    pub fn max_value(&self) -> T {
        self.max_value
    }

    pub fn add_values(&mut self, v: &[T; NUM_SERIES]) {
        // Our chart can only show positive values.  Filter out any negative numbers.
        let x = v.map(|n| {
            if n >= Default::default() {
                n
            } else {
                Default::default()
            }
        });

        // Update our stored maximum value, if these new data points are a new maximum,
        // or if the value we are expiring was the old maximum.
        let total: T = x.into_iter().sum();
        if total >= self.max_value {
            self.max_value = total;
            self.data[self.next_index] = x;
        } else {
            let expired_total: T = self.data[self.next_index].into_iter().sum();
            self.data[self.next_index] = x;
            if expired_total >= self.max_value {
                self.max_value = self.compute_max();
            }
        }

        self.next_index += 1;
        if self.next_index >= self.data.len() {
            self.next_index = 0;
        }
    }

    fn compute_max(&self) -> T {
        let mut max_value: T = Default::default();
        for &entry in &self.data {
            let total: T = entry.into_iter().sum();
            if total > max_value {
                max_value = total;
            }
        }
        max_value
    }

    pub fn draw(&self, cr: &cairo::Context, width: i32, height: i32, y_scale: f64) {
        let x_scale: f64 = 1.0;

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
                let value_f64: f64 = value.into_f64_lossy();
                let y = value_f64 * y_scale;
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
