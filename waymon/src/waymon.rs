use crate::bar::Bar;
use crate::config::{BarConfig, Config, MonitorRule, NO_BAR_NAME};
use crate::stats::AllStats;
use anyhow::Result;
use gtk::pango::EllipsizeMode;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/**
 * A singleton containing global state for the application
 */
pub struct Waymon {
    pub display: gdk::Display,
    config_dir: PathBuf,
    pub config: Config,
    timeout_id: Option<glib::source::SourceId>,
    monitors: HashMap<gdk::Monitor, MonitorState>,
    pub all_stats: AllStats,
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
            all_stats: AllStats::new(),
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
            info!(
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
                        warn!(
                            "timed out waiting for monitor metadata for monitor {}",
                            monitor_desc(mon)
                        );
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
        debug!("Monitor list changed");
        let mut changes_made = self.clean_up_removed_monitors(monitors);
        changes_made |= self.process_new_monitors(monitors, rc);
        if changes_made {
            self.configure_monitor_bars();
        }
    }

    fn clean_up_removed_monitors(&mut self, monitors: &gtk::gio::ListModel) -> bool {
        // Delete bars whose windows are no longer mapped.
        // This handles cleaning up bars for monitors that have been removed.
        let old_size = self.monitors.len();
        self.monitors
            .retain(|mon, _mon_state| Self::is_monitor_still_present(mon, monitors));
        old_size != self.monitors.len()
    }

    fn is_monitor_still_present(mon: &gdk::Monitor, monitors: &gtk::gio::ListModel) -> bool {
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
            info!("<- monitor removed: {}", monitor_desc(mon));
            /*
                if let MonitorState::Pending(_) = mon_state {
                    *mon_state = MonitorState::NoBar;
                }
            */
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
                debug!("-- existing monitor {}", monitor_desc(&mon));
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
                let handler_id = mon.connect_notify_local(None, move |mon_obj, _param_spec| {
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
        info!("--> new monitor {}", monitor_desc(mon));
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
        let primary_config = self.config.primary_bar();

        // Make sure a bar exists for every monitor
        for (mon, mon_state) in self.monitors.iter_mut() {
            Self::ensure_bar_config(mon, mon_state, Some(primary_config), &mut self.all_stats);
        }
    }

    fn ensure_bar_config(
        mon: &gdk::Monitor,
        mon_state: &mut MonitorState,
        config: Option<&BarConfig>,
        all_stats: &mut AllStats,
    ) {
        match mon_state {
            MonitorState::Pending(_) => {
                // We generally shouldn't be trying to configure a bar on a monitor that is still
                // waiting on metadata to populate
                error!("attempted to configure bar on pending monitor");
            }
            MonitorState::Bar(bar) => {
                if let Some(bar_config) = config {
                    bar.ensure_config(bar_config);
                } else {
                    // Clear the state.  The Bar drop() function will destroy the window.
                    *mon_state = MonitorState::NoBar;
                }
            }
            MonitorState::NoBar => {
                if let Some(bar_config) = config {
                    debug!("add bar for monitor {}", monitor_desc(mon));
                    let bar = Bar::new(mon.clone(), bar_config, all_stats);
                    *mon_state = MonitorState::Bar(bar);
                }
            }
        }
    }

    fn configure_monitors_primary(&mut self) {
        let primary_mon = self.pick_primary_monitor();
        let primary_config = self.config.primary_bar();

        for (mon, mon_state) in self.monitors.iter_mut() {
            if Some(mon) == primary_mon.as_ref() {
                Self::ensure_bar_config(mon, mon_state, Some(primary_config), &mut self.all_stats);
            } else {
                Self::ensure_bar_config(mon, mon_state, None, &mut self.all_stats);
            }
        }
    }

    fn pick_primary_monitor(&self) -> Option<gdk::Monitor> {
        let mut excluded_monitors = HashSet::<gdk::Monitor>::new();

        for rule in &self.config.monitor_rules {
            for mon in self.monitors.keys() {
                if excluded_monitors.contains(mon) {
                    continue;
                }
                if Self::is_rule_match(rule, mon) {
                    if let Some(bar_name) = &rule.bar {
                        if bar_name == NO_BAR_NAME {
                            // This monitor should be excluded from consideration
                            excluded_monitors.insert(mon.clone());
                        } else {
                            debug!("preferred primary monitor: {}", monitor_desc(mon));
                            return Some(mon.clone());
                        }
                    }
                }
            }
        }

        // No rules matched.
        // If one monitor already contains a bar, prefer using it rather than switching
        // the primary monitor to a different monitor.  Otherwise just return the first
        // non-excluded monitor, if there is one.
        let mut preferred: Option<&gdk::Monitor> = None;
        for (mon, mon_state) in &self.monitors {
            if excluded_monitors.contains(mon) {
                continue;
            }
            if let MonitorState::Bar(_) = mon_state {
                debug!(
                    "no matching preferred primary monitor, keeping current on: {}",
                    monitor_desc(mon)
                );
                return Some(mon.clone());
            } else if let None = preferred {
                preferred = Some(mon);
            }
        }
        debug!(
            "no matching preferred primary monitor, using: {}",
            preferred.map_or_else(|| "None".to_string(), |mon| monitor_desc(mon))
        );
        preferred.cloned()
    }

    fn is_rule_match(rule: &MonitorRule, mon: &gdk::Monitor) -> bool {
        let model_gstr = mon.model();
        let model: &str = model_gstr.as_ref().map_or("", |s| s.as_str());
        if !rule.model.is_match(model) {
            return false;
        }

        let mfgr_gstr = mon.manufacturer();
        let manufacturer: &str = mfgr_gstr.as_ref().map_or("", |s| s.as_str());
        if !rule.manufacturer.is_match(manufacturer) {
            return false;
        }

        let conn_gstr = mon.connector();
        let connector: &str = conn_gstr.as_ref().map_or("", |s| s.as_str());
        if !rule.connector.is_match(connector) {
            return false;
        }

        true
    }

    fn configure_monitors_per_monitor(&mut self) {
        for (mon, mon_state) in self.monitors.iter_mut() {
            if let MonitorState::Pending(_) = mon_state {
                // We are still waiting on metadata for this monitor to populate
                continue;
            }

            let bar_config = Self::pick_monitor_config(mon, &self.config);
            Self::ensure_bar_config(mon, mon_state, bar_config, &mut self.all_stats);
        }
    }

    fn pick_monitor_config<'a>(mon: &gdk::Monitor, config: &'a Config) -> Option<&'a BarConfig> {
        let conn_gstr = mon.connector();
        let mfgr_gstr = mon.manufacturer();
        let model_gstr = mon.model();
        let connector: &str = conn_gstr.as_ref().map_or("", |s| s.as_str());
        let manufacturer: &str = mfgr_gstr.as_ref().map_or("", |s| s.as_str());
        let model: &str = model_gstr.as_ref().map_or("", |s| s.as_str());

        for rule in &config.monitor_rules {
            if rule.model.is_match(model)
                && rule.manufacturer.is_match(manufacturer)
                && rule.connector.is_match(connector)
            {
                if let Some(cfg_name) = &rule.bar {
                    if cfg_name == NO_BAR_NAME {
                        debug!("monitor {} -> no bar", monitor_desc(mon));
                        return None;
                    }
                    debug!("monitor {} -> config {:?}", monitor_desc(mon), cfg_name);
                    return config.bars.get(cfg_name);
                } else {
                    // Default to the primary bar config
                    // We only use no bar if the name is explicitly "none"
                    debug!(
                        "monitor {} -> defaulting to primary bar config",
                        monitor_desc(mon)
                    );
                    return Some(config.primary_bar());
                }
            }
        }

        // Default to the primary monitor config if no other config specified
        Some(config.primary_bar())
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
    // TODO: report the parsing error in a GUI dialog rather than just in log output
    error!("CSS parsing error at {}: {}\n", section.to_str(), error);
}

fn monitor_desc(mon: &gdk::Monitor) -> String {
    format!(
        "{:?} {:?} {:?}",
        mon.connector(),
        mon.manufacturer(),
        mon.model()
    )
}
