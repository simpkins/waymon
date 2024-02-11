use anyhow::Result;
use gtk::pango::EllipsizeMode;
use gtk::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Duration;
use crate::config::{Config, CpuWidgetConfig, DiskIOWidgetConfig, WidgetConfig};

pub struct Waymon {
    config_dir: PathBuf,
    config: Config,
    timeout_id: Option<gtk::glib::source::SourceId>,
}

impl Waymon {
    pub fn new(config_dir: &Path) -> Result<Waymon> {
        let waymon = Waymon {
            config_dir: config_dir.to_path_buf(),
            config: Config::load(&config_dir.join("config.toml"))?,
            timeout_id: None,
        };
        Ok(waymon)
    }

    pub fn start(&mut self) {
        assert_eq!(self.timeout_id, None);
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
