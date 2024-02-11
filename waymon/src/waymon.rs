use crate::config::{Config, CpuWidgetConfig, DiskIOWidgetConfig, WidgetConfig};
use anyhow::Result;
use gtk::pango::EllipsizeMode;
use gtk::prelude::*;
use gtk::{glib, Orientation, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Waymon {
    config_dir: PathBuf,
    config: Config,
    timeout_id: Option<glib::source::SourceId>,
    window: Option<gtk::Window>,
}

fn on_tick() -> glib::ControlFlow {
    println!("tick!");
    glib::ControlFlow::Continue
}

impl Waymon {
    pub fn new(config_dir: &Path) -> Result<Waymon> {
        let waymon = Waymon {
            config_dir: config_dir.to_path_buf(),
            config: Config::load(&config_dir.join("config.toml"))?,
            timeout_id: None,
            window: None,
        };
        Ok(waymon)
    }

    pub fn start(&mut self) {
        assert_eq!(self.timeout_id, None);
        self.timeout_id = Some(glib::timeout_add(self.interval(), on_tick));
    }

    pub fn interval(&self) -> Duration {
        self.config.interval
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
        let window = Window::new();
        window.set_title(Some("waymon"));

        // Configure the window as a layer surface
        window.init_layer_shell();
        // Display below normal windows
        window.set_layer(Layer::Top);
        // Push other windows out of the way
        window.auto_exclusive_zone_enable();

        // Anchor to the right edge
        window.set_anchor(Edge::Right, true);
        // Anchor to both top and bottom edges, to span the entire height of the
        // screen.
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Bottom, true);

        window.set_default_size(100, -1);

        let box_widget = gtk::Box::new(Orientation::Vertical, /*spacing*/ 0);
        box_widget.add_css_class("background");
        window.set_child(Some(&box_widget));

        let css = gtk::CssProvider::new();
        css.connect_parsing_error(report_css_parsing_error);
        css.load_from_path(self.css_path());
        gtk::style_context_add_provider_for_display(
            &WidgetExt::display(&window),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        self.add_widgets(&box_widget);

        // Present window
        window.present();
        self.window = Some(window);
    }

    pub fn add_widgets(&self, container: &gtk::Box) {
        for w in &self.config.widgets {
            match w {
                WidgetConfig::Cpu(cpu) => self.add_cpu_widget(cpu, container),
                WidgetConfig::DiskIO(disk) => self.add_disk_io_widget(disk, container),
            }
        }
    }

    fn add_cpu_widget(&self, config: &CpuWidgetConfig, container: &gtk::Box) {
        self.add_widget_label(&config.label, container);
    }

    fn add_disk_io_widget(&self, config: &DiskIOWidgetConfig, container: &gtk::Box) {
        self.add_widget_label(&config.label, container);
        println!("disk widget for: {:?}", config.disk);
    }

    fn add_widget_label(&self, text: &str, container: &gtk::Box) {
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
}

impl Drop for Waymon {
    fn drop(&mut self) {
        if let Some(t) = self.timeout_id.take() {
            t.remove();
        }
        if let Some(win) = self.window.take() {
            win.destroy();
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
