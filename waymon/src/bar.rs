use crate::config::{BarConfig, WidgetConfig};
use crate::stats::AllStats;
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
    pub window: Window,
    pub monitor: gdk::Monitor,
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
    pub fn new(monitor: gdk::Monitor, config: &BarConfig, all_stats: &mut AllStats) -> Self {
        let (window, box_widget) = Self::create_window(&monitor, config);
        let mut bar = Self {
            window,
            monitor,
            box_widget,
            widgets: Vec::new(),
        };

        // Add the widgets
        bar.add_widgets(config, config.width, all_stats);
        // Display the window
        bar.window.present();

        bar
    }

    pub fn ensure_config(&mut self, _config: &BarConfig) {
        // TODO: reconfigure the bar if needed
        eprintln!("TODO: update bar config");
    }

    fn create_window(monitor: &gdk::Monitor, config: &BarConfig) -> (Window, gtk::Box) {
        let display = monitor.display();
        let window = Window::builder().display(&display).title("waymon").build();

        // Configure the window as a layer surface
        window.init_layer_shell();
        // Set the monitor it will display on
        window.set_monitor(&monitor);
        // Display below normal windows
        window.set_layer(Layer::Top);
        // Push other windows out of the way
        window.auto_exclusive_zone_enable();

        let box_orientation = match config.side {
            crate::config::Side::Left => {
                window.set_anchor(Edge::Left, true);
                window.set_anchor(Edge::Top, true);
                window.set_anchor(Edge::Bottom, true);
                Orientation::Vertical
            }
            crate::config::Side::Right => {
                window.set_anchor(Edge::Right, true);
                window.set_anchor(Edge::Top, true);
                window.set_anchor(Edge::Bottom, true);
                Orientation::Vertical
            }
            crate::config::Side::Top => {
                window.set_anchor(Edge::Top, true);
                window.set_anchor(Edge::Left, true);
                window.set_anchor(Edge::Right, true);
                Orientation::Horizontal
            }
            crate::config::Side::Bottom => {
                window.set_anchor(Edge::Bottom, true);
                window.set_anchor(Edge::Left, true);
                window.set_anchor(Edge::Right, true);
                Orientation::Horizontal
            }
        };
        let box_widget = gtk::Box::new(box_orientation, /*spacing*/ 0);

        if box_orientation == Orientation::Vertical {
            window.set_default_size(config.width as i32, -1);
        } else {
            // TODO: we should do something better for setting the window height.
            // It is confusing to use the "width" setting for this.  We should perhaps iterate
            // through all the widgets and pick the max height.  Perhaps just setting a small value
            // here would be fine, and then the widgets should cause the bar to expand?
            window.set_default_size(-1, config.width as i32);
        }

        box_widget.add_css_class("background");
        window.set_child(Some(&box_widget));

        (window, box_widget)
    }

    fn add_widgets(&mut self, config: &BarConfig, width: u32, all_stats: &mut AllStats) {
        // Our charts generally display one pixel per data point.
        // Store history for exactly as many data points as we have pixels wide.
        let history_length: usize = width as usize;

        let container = &self.box_widget;
        for widget_config in &config.widgets {
            let widget: Rc<RefCell<dyn Widget>> = match widget_config {
                WidgetConfig::Cpu(cpu) => CpuWidget::new(container, cpu, all_stats, history_length),
                WidgetConfig::DiskIO(disk) => {
                    DiskIoWidget::new(container, disk, all_stats, history_length)
                }
                WidgetConfig::Net(net) => NetWidget::new(container, net, all_stats, history_length),
                WidgetConfig::Mem(mem) => MemWidget::new(container, mem, all_stats, history_length),
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

impl Drop for Bar {
    fn drop(&mut self) {
        eprintln!(
            "bar for monitor {:?} {:?} dropped",
            self.monitor.manufacturer(),
            self.monitor.model()
        );
        self.window.destroy()
    }
}
