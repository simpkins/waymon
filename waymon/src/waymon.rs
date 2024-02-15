use crate::config::{Config, WidgetConfig};
use crate::widgets::cpu::CpuWidget;
use crate::widgets::disk_io::DiskIoWidget;
use crate::widgets::Widget;
use anyhow::Result;
use gtk::pango::EllipsizeMode;
use gtk::prelude::*;
use gtk::{glib, Orientation, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

pub struct Waymon {
    config_dir: PathBuf,
    config: Config,
    timeout_id: Option<glib::source::SourceId>,
    window: Window,
    last_update: Instant,
    // It's sort of annoying that we have to store each widget in an Rc<RefCell>, given that the
    // entire Waymon structure itself is also in a Rc<RefCell> and only one operation ever happens
    // at a time.  It would be nicer if we could have only the single top-level Rc<RefCell>, and
    // each callback only had to try borrowing from that.  Unfortunately, there doesn't seem to be
    // a good way to express this currently with Rust.  We pay the cost of doing some extra
    // unnecessary runtime borrow checks as a result.
    widgets: Vec<Rc<RefCell<dyn Widget>>>,
    all_stats: crate::stats::AllStats,
}

pub struct WaymonState {
    cell: Rc<RefCell<Waymon>>,
}

impl WaymonState {
    pub fn new(config_dir: &Path) -> Result<WaymonState> {
        let waymon = Waymon::new(&config_dir)?;
        Ok(WaymonState {
            cell: Rc::new(RefCell::new(waymon)),
        })
    }

    pub fn start(&self) {
        let mut waymon = self.cell.borrow_mut();
        waymon.create_window();
        waymon.last_update = Instant::now();

        assert_eq!(waymon.timeout_id, None);
        let new_ref = self.cell.clone();
        waymon.timeout_id = Some(glib::timeout_add_local(waymon.config.interval, move || {
            Self::on_tick(&new_ref)
        }));
    }

    fn on_tick(waymon_cell: &Rc<RefCell<Waymon>>) -> glib::ControlFlow {
        let mut waymon = waymon_cell.borrow_mut();

        let old_interval = waymon.config.interval;
        waymon.on_tick();
        let new_interval = waymon.config.interval;
        if new_interval != old_interval {
            eprintln!(
                "update interval from {:?} to {:?}",
                old_interval, new_interval
            );
            let new_ref = waymon_cell.clone();
            waymon.timeout_id = Some(glib::timeout_add_local(new_interval, move || {
                Self::on_tick(&new_ref)
            }));
            return glib::ControlFlow::Break;
        }

        glib::ControlFlow::Continue
    }
}

impl Waymon {
    pub fn new(config_dir: &Path) -> Result<Waymon> {
        let waymon = Waymon {
            config_dir: config_dir.to_path_buf(),
            config: Config::load(&config_dir.join("config.toml"))?,
            timeout_id: None,
            window: Window::new(),
            last_update: Instant::now(),
            widgets: Vec::new(),
            all_stats: crate::stats::AllStats::new(),
        };
        Ok(waymon)
    }

    pub fn css_path(&self) -> PathBuf {
        self.config_dir.join("style.css")
    }

    /*
    pub fn reload_config(&mut self) -> Result<()> {
        let config_path = self.toml_config_path();
        self.config = Config::load(&config_path)?;
        Ok(())
    }
    */

    pub fn create_window(&mut self) {
        // Create a window and set the title
        self.window.set_title(Some("waymon"));

        // Configure the window as a layer surface
        self.window.init_layer_shell();
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

        self.window.set_default_size(self.config.width as i32, -1);

        let box_widget = gtk::Box::new(Orientation::Vertical, /*spacing*/ 0);
        box_widget.add_css_class("background");
        self.window.set_child(Some(&box_widget));

        let css = gtk::CssProvider::new();
        css.connect_parsing_error(report_css_parsing_error);
        css.load_from_path(self.css_path());
        gtk::style_context_add_provider_for_display(
            &WidgetExt::display(&self.window),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        self.add_widgets(&box_widget);

        // Present window
        self.window.present();
    }

    pub fn add_widgets(&mut self, container: &gtk::Box) {
        // Our charts generally display one pixel per data point.
        // Store history for exactly as many data points as we have pixels wide.
        let history_length: usize = self.config.width as usize;

        for widget_config in &self.config.widgets {
            let widget: Rc<RefCell<dyn Widget>> = match widget_config {
                WidgetConfig::Cpu(cpu) => {
                    CpuWidget::new(container, cpu, &mut self.all_stats, history_length)
                }
                WidgetConfig::DiskIO(disk) => {
                    DiskIoWidget::new(container, disk, &mut self.all_stats, history_length)
                }
            };
            self.widgets.push(widget);
        }
    }

    pub fn add_widget_label(container: &gtk::Box, text: &str) {
        let label = gtk::Label::new(None);
        label.add_css_class("chart-header");
        label.set_markup(&format!("<span font_desc=\"12.0\">{}</span>", text));
        // If the label text is very long,
        // truncate it rather than expanding the width of the widget.
        label.set_width_chars(1);
        label.set_hexpand(true);
        label.set_ellipsize(EllipsizeMode::End);
        container.append(&label);
    }

    fn on_tick(&mut self) {
        let now = Instant::now();
        self.all_stats.update(now);
        for w_rc in &self.widgets {
            let mut w = w_rc.borrow_mut();
            w.update();
        }
    }
}

fn report_css_parsing_error(
    _css: &gtk::CssProvider,
    section: &gtk::CssSection,
    error: &glib::Error,
) {
    eprintln!("CSS parsing error at {}: {}\n", section.to_str(), error);
}
