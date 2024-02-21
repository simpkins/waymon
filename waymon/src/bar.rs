use crate::config::WidgetConfig;
use crate::waymon::Waymon;
use crate::widgets::cpu::CpuWidget;
use crate::widgets::disk_io::DiskIoWidget;
use crate::widgets::mem::MemWidget;
use crate::widgets::net::NetWidget;
use crate::widgets::Widget;
use gtk::gdk;
use gtk::prelude::*;
use gtk::{Orientation, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;

/**
 * A Bar is a single waymon window, containing a set of chart widgets.
 *
 * Each Bar is associated with a single monitor (aka wayland output).  In general each monitor will
 * have either 0 or 1 Bar.
 */
pub struct Bar {
    window: Window,
    box_widget: gtk::Box,
    // It's sort of annoying that we have to store each widget in an Rc<RefCell>, given that the
    // entire Waymon structure itself is also in a Rc<RefCell> and only one operation ever happens
    // at a time.  It would be nicer if we could have only the single top-level Rc<RefCell>, and
    // each callback only had to try borrowing from that.  Unfortunately, there doesn't seem to be
    // a good way to express this currently with Rust.  We pay the cost of doing some extra
    // unnecessary runtime borrow checks as a result.
    widgets: Vec<Rc<RefCell<dyn Widget>>>,
}

impl Bar {
    pub fn new(waymon: &mut Waymon, monitor: &gdk::Monitor) -> Self {
        let window = Window::builder()
            .display(&waymon.display)
            .title("waymon")
            .build();

        let mut bar = Self {
            window: window,
            box_widget: gtk::Box::new(Orientation::Vertical, /*spacing*/ 0),
            widgets: Vec::new(),
        };
        bar.create_window(waymon, monitor);
        bar
    }

    fn create_window(&mut self, waymon: &mut Waymon, monitor: &gdk::Monitor) {
        // Configure the window as a layer surface
        self.window.init_layer_shell();
        // Set the monitor it will display on
        self.window.set_monitor(monitor);
        // Display below normal windows
        self.window.set_layer(Layer::Top);
        // Push other windows out of the way
        self.window.auto_exclusive_zone_enable();

        // Anchor to the right edge
        self.window.set_anchor(Edge::Right, true);
        // Anchor to both top and bottom edges, to span the entire height of the
        // screen.
        self.window.set_anchor(Edge::Top, true);
        self.window.set_anchor(Edge::Bottom, true);

        self.window.set_default_size(waymon.config.width as i32, -1);

        self.box_widget.add_css_class("background");
        self.window.set_child(Some(&self.box_widget));

        self.add_widgets(waymon);

        // Present window
        self.window.present();
    }

    fn add_widgets(&mut self, waymon: &mut Waymon) {
        // Our charts generally display one pixel per data point.
        // Store history for exactly as many data points as we have pixels wide.
        let history_length: usize = waymon.config.width as usize;

        let container = &self.box_widget;
        for widget_config in &waymon.config.widgets {
            let widget: Rc<RefCell<dyn Widget>> = match widget_config {
                WidgetConfig::Cpu(cpu) => {
                    CpuWidget::new(container, cpu, &mut waymon.all_stats, history_length)
                }
                WidgetConfig::DiskIO(disk) => {
                    DiskIoWidget::new(container, disk, &mut waymon.all_stats, history_length)
                }
                WidgetConfig::Net(net) => {
                    NetWidget::new(container, net, &mut waymon.all_stats, history_length)
                }
                WidgetConfig::Mem(mem) => {
                    MemWidget::new(container, mem, &mut waymon.all_stats, history_length)
                }
            };
            self.widgets.push(widget);
        }
    }

    pub fn update(&self) {
        for w_rc in &self.widgets {
            let mut w = w_rc.borrow_mut();
            w.update();
        }
    }
}
