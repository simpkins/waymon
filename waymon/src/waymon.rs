use crate::bar::Bar;
use crate::config::{BarConfig, Config};
use anyhow::Result;
use gtk::pango::EllipsizeMode;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

/**
 * A singleton containing global state for the application
 */
pub struct Waymon {
    pub display: gdk::Display,
    config_dir: PathBuf,
    pub config: Config,
    timeout_id: Option<glib::source::SourceId>,
    monitors: HashMap<gdk::Monitor, MonitorState>,
    pub all_stats: crate::stats::AllStats,
}

/**
 * A helper class that just wraps an Rc<RefCell<Waymon>>
 */
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
            monitors: HashMap::new(),
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
        let rc_clone = rc.clone();
        monitors.connect_items_changed(move |monitors, _position, _num_removed, _num_added| {
            let mut waymon = rc_clone.borrow_mut();
            waymon.process_monitor_change(monitors, &rc_clone);
        });
        self.process_monitor_change(&monitors, &rc);
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

        // Update the bars on all monitors
        let mut monitors_changed = false;
        for (mon, mon_state) in self.monitors.iter_mut() {
            match mon_state {
                MonitorState::Pending(pm) => {
                    // We are still waiting on metadata for this monitor to populate.
                    // If it has been too long, time out and process the monitor with the metadata
                    // we do have.
                    if pm.is_timed_out(&now) {
                        eprintln!("timed out waiting for monitor metadata");
                        *mon_state = MonitorState::NoBar;
                        monitors_changed = true;
                    }
                }
                MonitorState::Bar(bar) => bar.update(),
                MonitorState::NoBar => (),
            };
        }

        if monitors_changed {
            self.configure_monitor_bars();
        }
    }

    fn process_monitor_change(&mut self, monitors: &gtk::gio::ListModel, rc: &Rc<RefCell<Waymon>>) {
        eprintln!("Monitor list changed");
        let mut changes_made = self.clean_up_removed_monitors(monitors);
        changes_made |= self.process_new_monitors(monitors, rc);
        self.configure_monitor_bars();
    }

    fn clean_up_removed_monitors(&mut self, monitors: &gtk::gio::ListModel) -> bool {
        // Delete bars whose windows are no longer mapped.
        // This handles cleaning up bars for monitors that have been removed.
        let old_size = self.monitors.len();
        self.monitors
            .retain(|mon, mon_state| Self::is_monitor_still_present(mon, mon_state, monitors));
        /*
        self.monitors.retain(|mon, mon_state| match mon_state {
            MonitorState::Bar(bar) => {
                if bar.window.is_mapped() {
                    true
                } else {
                    eprintln!(
                        "- unmapped bar on {:?} {:?}",
                        mon.manufacturer(),
                        mon.model()
                    );
                    false
                }
            }
            _ => {
                let present = Self::is_monitor_in_list(mon, monitors);
                if !present {
                    eprintln!(
                        "- removed monitor {:?} {:?}",
                        mon.manufacturer(),
                        mon.model()
                    );
                }
                present
            }
        });
        */
        old_size == self.monitors.len()
    }

    fn is_monitor_still_present(
        mon: &gdk::Monitor,
        mon_state: &mut MonitorState,
        monitors: &gtk::gio::ListModel,
    ) -> bool {
        // We could check mon.is_valid(), but unfortunately this property is not updated until
        // after the monitor is removed from the list, so it still returns true here even for
        // removed monitors.
        //
        // The monitor's bar gets unmapped when the monitor is removed, so if a bar was present on
        // the monitor, checking bar.window.is_mapped() is a good way to detect if a monitor has
        // been removed.  However, this only works we had a bar on this monitor.
        //
        // Therefore we check is_monitor_in_list(), even though this requires a linear scan.
        let present = Self::is_monitor_in_list(mon, monitors);
        if !present {
            eprintln!(
                "<- monitor removed: {:?} {:?}",
                mon.manufacturer(),
                mon.model()
            );
            if let MonitorState::Pending(_) = mon_state {
                *mon_state = MonitorState::NoBar;
            }
        }
        present
    }

    fn is_monitor_in_list(to_find: &gdk::Monitor, monitors: &gtk::gio::ListModel) -> bool {
        for mon_result in monitors.iter::<gdk::Monitor>() {
            let mon = match mon_result {
                Ok(mon) => mon,
                Err(_) => {
                    return false;
                }
            };
            if mon == *to_find {
                return true;
            }
        }
        false
    }

    fn process_new_monitors(
        &mut self,
        monitors: &gtk::gio::ListModel,
        my_rc: &Rc<RefCell<Waymon>>,
    ) -> bool {
        let mut changes_made = false;
        for mon_result in monitors.iter::<gdk::Monitor>() {
            let mon = match mon_result {
                Ok(mon) => mon,
                Err(_) => {
                    // The only error that can occur here is ListModelMutatedDuringIter
                    // If the list is mutated while we are iterating over it,
                    // process_monitor_change will be called again and we will examine the whole
                    // monitor list again from the start.  Return here, and let the next
                    // process_monitor_change() finish any remaining work.
                    return changes_made;
                }
            };

            if let Some(_) = self.monitors.get(&mon) {
                // TODO: perhaps make sure the bar is using up-to-date configuration info?
                eprintln!("-- existing monitor {:?}", mon.connector());
                continue;
            }

            if Self::is_all_monitor_metadata_present(&mon) {
                // We set the state to NoBar for now.
                // Afterwards we will process all monitors and decide the correct bar configuration
                // for each one.
                self.monitors.insert(mon, MonitorState::NoBar);
                changes_made = true;
            } else {
                // We don't have the metadata for this monitor yet.
                // GTK unfortunately reports new monitors before it has the metadata,
                // so we have to wait until the metadata is available before we can process this.
                let rc_clone = my_rc.clone();
                let handler_id = mon.connect_notify_local(None, move |mon_obj, param_spec| {
                    if Self::is_all_monitor_metadata_present(mon_obj) {
                        let mut waymon = rc_clone.borrow_mut();
                        waymon.monitor_metadata_ready(mon_obj);
                    }
                });
                let mon2 = mon.clone();
                self.monitors.insert(
                    mon,
                    MonitorState::Pending(PendingMonitor::new(mon2, handler_id)),
                );
            }
        }

        changes_made
    }

    fn is_all_monitor_metadata_present(mon: &gdk::Monitor) -> bool {
        mon.connector().is_some() && mon.manufacturer().is_some() && mon.model().is_some()
    }

    fn monitor_metadata_ready(&mut self, mon: &gdk::Monitor) {
        // In theory we presumably only get here if we have a Pending entry for this monitor.
        // We want to replace that with a NoBar entry.  Just to be safe, check to see what
        // information we have, and don't replace an existing entry if we somehow already have a
        // bar present on this monitor.
        eprintln!("all metadata ready for monitor {:?} {:?}", mon.manufacturer(), mon.model());
        if let Some(mon_state) = self.monitors.get_mut(mon) {
            if let MonitorState::Pending(_) = mon_state {
                self.monitors.insert(mon.clone(), MonitorState::NoBar);
            }
        } else {
            self.monitors.insert(mon.clone(), MonitorState::NoBar);
        }
        self.configure_monitor_bars();
    }

    /**
     * Make sure each monitor is showing a bar with the correct configuration
     */
    fn configure_monitor_bars(&mut self) {
        match self.config.mode {
            crate::config::Mode::Mirror => self.configure_monitors_mirrored(),
            crate::config::Mode::Primary => self.configure_monitors_primary(),
            crate::config::Mode::PerMonitor => self.configure_monitors_per_monitor(),
        }
    }

    fn configure_monitors_mirrored(&mut self) {
        // TODO: we perhaps should have a real fallback config with a basic set of widgets
        let fallback_config =
            toml::from_str::<BarConfig>("").expect("deserialization of literal should not fail");
        let primary_config = self.config.bars.get("primary").unwrap_or_else(|| {
            eprintln!("no widgets defined for the primary bar!");
            &fallback_config
        });

        // Make sure a bar exists for every monitor
        for (mon, mon_state) in self.monitors.iter_mut() {
            match mon_state {
                MonitorState::Pending(_) => {
                    // Ignore monitors that don't have all metadata yet
                }
                MonitorState::Bar(bar) => {
                    // TODO
                    // bar.ensure_config(primary_config, self.config.width);
                }
                MonitorState::NoBar => {
                    let bar = Bar::new(
                        mon.clone(),
                        primary_config,
                        self.config.width,
                        &mut self.all_stats,
                    );
                    eprintln!(
                        "add bar for monitor {:?} {:?}",
                        mon.manufacturer(),
                        mon.model()
                    );
                    *mon_state = MonitorState::Bar(bar);
                }
            }
        }
    }

    fn configure_monitors_primary(&mut self) {
        /*
        let primary_mon = self.pick_primary_monitor();
        let primary_config = self.get_primary_bar_config();

        for (mon, mon_info) in &self.monitors {
            if let Some(_) = mon_info.signal_handler {
                // We are still waiting on metadata for this monitor to populate
                continue;
            }
            if mon == primary_mon {
                mon_info.ensure_bar_config(primary_config);
            } else {
                mon_info.ensure_bar_config(None);
            }
        }
        */
    }

    fn configure_monitors_per_monitor(&mut self) {
        /*
            for (mon, mon_info) in &self.monitors {
                if let Some(_) = mon_info.signal_handler {
                    // We are still waiting on metadata for this monitor to populate
                    continue;
                }

                let config_opt = self.pick_monitor_config(mon, mon_info);
                mon_info.ensure_bar_config(config_opt);
            }
        */
    }
}

enum MonitorState {
    // This is a new monitor, and we are waiting for more information about it before we can
    // decide what sort of bar should be displayed on this monitor.
    Pending(PendingMonitor),
    // No bar is shown on this monitor.
    NoBar,
    // The bar being shown on this monitor.
    Bar(Bar),
}

struct PendingMonitor {
    mon: gdk::Monitor,
    first_seen: Instant,
    signal_handler: Option<glib::SignalHandlerId>,
}

impl PendingMonitor {
    fn new(mon: gdk::Monitor, handler_id: glib::SignalHandlerId) -> Self {
        Self {
            mon: mon,
            first_seen: Instant::now(),
            signal_handler: Some(handler_id),
        }
    }

    fn is_timed_out(&self, now: &Instant) -> bool {
        const MONITOR_METADATA_TIMEOUT: Duration = Duration::from_secs(5);
        *now > self.first_seen + MONITOR_METADATA_TIMEOUT
    }
}

impl Drop for PendingMonitor {
    fn drop(&mut self) {
        // Remove the monitor metadata callback once we have stopped waiting on this monitor
        if let Some(sh) = std::mem::take(&mut self.signal_handler) {
            self.mon.disconnect(sh);
        }
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
