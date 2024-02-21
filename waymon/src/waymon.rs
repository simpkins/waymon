use crate::bar::Bar;
use crate::config::Config;
use anyhow::Result;
use gtk::pango::EllipsizeMode;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

pub struct Waymon {
    pub display: gdk::Display,
    config_dir: PathBuf,
    pub config: Config,
    timeout_id: Option<glib::source::SourceId>,
    bars: Vec<Bar>,
    pub all_stats: crate::stats::AllStats,
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
        waymon.start(self.cell.clone());
    }
}

impl Waymon {
    pub fn new(config_dir: &Path) -> Result<Waymon> {
        let waymon = Waymon {
            display: gdk::Display::default()
                .ok_or_else(|| anyhow::anyhow!("failed to get GTK display"))?,
            config_dir: config_dir.to_path_buf(),
            config: Config::load(&config_dir.join("config.toml"))?,
            timeout_id: None,
            bars: Vec::new(),
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

    pub fn start(&mut self, rc: Rc<RefCell<Waymon>>) {
        let css = gtk::CssProvider::new();
        css.connect_parsing_error(report_css_parsing_error);
        css.load_from_path(self.css_path());
        gtk::style_context_add_provider_for_display(
            &self.display,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        self.create_bars(rc.clone());
        self.start_timeout(rc);
    }

    fn create_bars(&mut self, rc: Rc<RefCell<Waymon>>) {
        let monitors = self.display.monitors();
        let mut mon_idx = 0;
        loop {
            let mon = match monitors
                .item(mon_idx)
                .and_then(|mon_obj| mon_obj.downcast::<gdk::Monitor>().ok())
            {
                Some(m) => m,
                None => break,
            };

            eprintln!(
                "- monitor {}: conn={:?} model={:?} mfgr={:?}",
                mon_idx,
                mon.connector(),
                mon.model(),
                mon.manufacturer(),
            );
            let bar = Bar::new(self, &mon);
            self.bars.push(bar);
            mon_idx += 1;
        }

        monitors.connect_items_changed(move |monitors, _position, _num_removed, _num_added| {
            eprintln!(
                "monitors notify: monitors={:?} pos={:?} num_removed={:?} num_added={:?}",
                monitors, _position, _num_removed, _num_added
            );
            let mut waymon = rc.borrow_mut();
            waymon.process_monitor_change();
        });
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

    fn start_timeout(&mut self, rc: Rc<RefCell<Waymon>>) {
        assert_eq!(self.timeout_id, None);
        self.timeout_id = Some(glib::timeout_add_local(self.config.interval, move || {
            Self::on_tick_callback(&rc)
        }));
    }

    fn on_tick_callback(rc: &Rc<RefCell<Waymon>>) -> glib::ControlFlow {
        let mut waymon = rc.borrow_mut();

        let old_interval = waymon.config.interval;
        waymon.process_tick();
        let new_interval = waymon.config.interval;
        if new_interval != old_interval {
            eprintln!(
                "update interval from {:?} to {:?}",
                old_interval, new_interval
            );
            let new_ref = rc.clone();
            waymon.timeout_id = Some(glib::timeout_add_local(new_interval, move || {
                Self::on_tick_callback(&new_ref)
            }));
            return glib::ControlFlow::Break;
        }

        glib::ControlFlow::Continue
    }

    fn process_tick(&mut self) {
        let now = Instant::now();
        self.all_stats.update(now);

        // TODO: check if config file or css file has been updated,
        // and reload if needed

        for bar in &self.bars {
            bar.update();
        }
    }

    fn process_monitor_change(&mut self) {
        eprintln!("process_monitor_change");
        // TODO: update self.bars based on the new set of monitors
    }
}

fn report_css_parsing_error(
    _css: &gtk::CssProvider,
    section: &gtk::CssSection,
    error: &glib::Error,
) {
    // TODO: report the parsing error in a GUI dialog rather than just in stderr output
    eprintln!("CSS parsing error at {}: {}\n", section.to_str(), error);
}
