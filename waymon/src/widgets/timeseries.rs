use gtk::cairo;
use gtk::prelude::*;

pub struct TimeseriesChart {
    // annotation: String,
}

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
