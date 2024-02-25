use crate::widgets::cpu::CpuWidget;
use crate::widgets::disk_io::DiskIoWidget;
use crate::widgets::mem::MemWidget;
use crate::widgets::net::NetWidget;
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use waymon_widget_derive::WaymonWidgetConfig;

const DEFAULT_WIDTH: u32 = 100;
const DEFAULT_SIDE: Side = Side::Right;
const PRIMARY_BAR_NAME: &str = "primary";
pub const NO_BAR_NAME: &str = "none";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    // Show the "primary" bar configuration on all monitors
    Mirror,
    // Show the "primary" bar config on one monitor only
    // The monitor_rules will be used to select the primary monitor
    Primary,
    // Use the monitor
    PerMonitor,
}

#[derive(Debug)]
pub struct Config {
    pub mode: Mode,
    pub interval: Duration,
    pub monitor_rules: Vec<MonitorRule>,
    pub bars: HashMap<String, BarConfig>,
}

impl Config {
    pub fn primary_bar<'a>(&'a self) -> &'a BarConfig {
        // TomlConfig.to_config() should ensure that there is always a primary config entry
        self.bars.get(PRIMARY_BAR_NAME).unwrap()
    }
}

#[derive(Debug, Deserialize)]
pub struct TomlConfig {
    #[serde(default = "default_mode")]
    pub mode: Mode,
    #[serde(default = "default_interval")]
    #[serde(deserialize_with = "parse_duration")]
    pub interval: Duration,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_side")]
    pub side: Side,
    #[serde(default, rename = "monitor_rule")]
    pub monitor_rules: Vec<TomlMonitorRule>,
    #[serde(default, rename = "bar")]
    pub bars: HashMap<String, TomlBarConfig>,
    #[serde(default, rename = "widget")]
    pub widgets: Vec<WidgetConfig>,
}

impl TomlConfig {
    fn to_config(mut self) -> Result<Config> {
        // Ensure that a primary bar config exists
        // For convenience we also let widgets for the primary config be specified at the top-level
        // of the config file.  Bail out with an error if the user specified widgets both at the
        // top level and in an explicity "primary" bar config.
        if let Some(bc) = self.bars.get_mut(PRIMARY_BAR_NAME) {
            if !self.widgets.is_empty() {
                if !bc.widgets.is_empty() {
                    return Err(anyhow!(
                        "primary bar widgets specified both as [[widget]] and \
                        [[bar.primary.widget]].  Choose one config style or the other, not both"
                    ));
                }
                bc.widgets = self.widgets
            }
        } else if !self.widgets.is_empty() {
            self.bars.insert(
                PRIMARY_BAR_NAME.to_string(),
                TomlBarConfig {
                    widgets: self.widgets,
                    ..Default::default()
                },
            );
        } else {
            self.bars.insert(
                PRIMARY_BAR_NAME.to_string(),
                TomlBarConfig {
                    widgets: default_widgets(),
                    ..Default::default()
                },
            );
        }

        // Convert the bar configs
        let bars: HashMap<String, BarConfig> = self
            .bars
            .drain()
            .map(|(name, bc)| (name, bc.to_config(self.width, self.side)))
            .collect();
        if bars.contains_key(NO_BAR_NAME) {
            return Err(anyhow!(
                "invalid bar name {}: this name is reserved",
                NO_BAR_NAME
            ));
        }

        Ok(Config {
            mode: self.mode,
            interval: self.interval,
            monitor_rules: Self::convert_monitor_rules(&mut self.monitor_rules, &bars)?,
            bars: bars,
        })
    }

    fn convert_monitor_rules(
        toml_rules: &mut Vec<TomlMonitorRule>,
        bars: &HashMap<String, BarConfig>,
    ) -> Result<Vec<MonitorRule>> {
        let parse_regex = |s: Option<String>| -> Result<Regex> {
            s.map_or(Ok(Regex::new(".*")?), |s| Ok(Regex::new(&s)?))
        };

        let mut result = Vec::new();
        for rule in toml_rules.drain(..) {
            if let Some(bar_name) = &rule.bar {
                // Check validity of the bar config name
                if bar_name != NO_BAR_NAME && bar_name != PRIMARY_BAR_NAME {
                    if !bars.contains_key(bar_name) {
                        return Err(anyhow!(
                            "monitor rule contains unknown bar name {:?}",
                            bar_name
                        ));
                    }
                }
            }

            let out_rule = MonitorRule {
                connector: parse_regex(rule.connector)?,
                manufacturer: parse_regex(rule.manufacturer)?,
                model: parse_regex(rule.model)?,
                bar: rule.bar,
            };
            result.push(out_rule);
        }
        Ok(result)
    }
}

fn default_widgets() -> Vec<WidgetConfig> {
    // Provide a default set of widgets so that the bar isn't completely empty by default
    // if the config file is empty.
    vec![
        WidgetConfig::Cpu(CpuWidgetConfig {
            label: "CPU".to_string(),
            height: default_chart_height(),
        }),
        WidgetConfig::Mem(MemWidgetConfig {
            label: "Memory".to_string(),
            height: default_chart_height(),
        }),
    ]
}

fn default_mode() -> Mode {
    Mode::Mirror
}

fn default_interval() -> Duration {
    Duration::from_secs(1)
}

fn default_width() -> u32 {
    DEFAULT_WIDTH
}

fn default_side() -> Side {
    DEFAULT_SIDE
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        let config_contents = read_config_contents(path)?;
        let toml_config = toml::from_str::<TomlConfig>(&config_contents)
            .with_context(|| format!("{}", path.display()))?;
        toml_config.to_config()
    }
}

#[derive(Debug)]
pub struct MonitorRule {
    pub connector: Regex,
    pub manufacturer: Regex,
    pub model: Regex,
    pub bar: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TomlMonitorRule {
    #[serde(default)]
    pub connector: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    pub bar: Option<String>,
}

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Right,
    Left,
    Top,
    Bottom,
}

#[derive(Debug)]
pub struct BarConfig {
    pub width: u32,
    pub side: Side,
    pub widgets: Vec<WidgetConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TomlBarConfig {
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub side: Option<Side>,
    #[serde(default, rename = "widget")]
    pub widgets: Vec<WidgetConfig>,
}

impl TomlBarConfig {
    fn to_config(self, default_width: u32, default_side: Side) -> BarConfig {
        BarConfig {
            width: self.width.unwrap_or(default_width),
            side: self.side.unwrap_or(default_side),
            widgets: self.widgets,
        }
    }
}

pub trait WaymonWidgetConfig {
    fn create_widget(
        &self,
        all_stats: &mut crate::stats::AllStats,
        history_length: usize,
    ) -> std::rc::Rc<std::cell::RefCell<dyn crate::widgets::Widget>>;
}

#[derive(Debug, Deserialize, WaymonWidgetConfig)]
#[serde(tag = "type")]
pub enum WidgetConfig {
    #[serde(rename = "cpu")]
    Cpu(CpuWidgetConfig),
    #[serde(rename = "disk_io")]
    DiskIO(DiskIoWidgetConfig),
    #[serde(rename = "net")]
    Net(NetWidgetConfig),
    #[serde(rename = "mem")]
    Mem(MemWidgetConfig),
}

fn default_chart_height() -> u32 {
    100
}

#[derive(Debug, Deserialize, WaymonWidgetConfig)]
pub struct CpuWidgetConfig {
    pub label: String,

    #[serde(default = "default_chart_height")]
    pub height: u32,
}

#[derive(Debug, Deserialize, WaymonWidgetConfig)]
pub struct DiskIoWidgetConfig {
    pub label: String,
    pub disk: String,

    #[serde(default = "default_chart_height")]
    pub height: u32,
}

#[derive(Debug, Deserialize, WaymonWidgetConfig)]
pub struct NetWidgetConfig {
    pub label: String,
    pub dev: String,

    #[serde(default = "default_chart_height")]
    pub height: u32,
}

#[derive(Debug, Deserialize, WaymonWidgetConfig)]
pub struct MemWidgetConfig {
    pub label: String,

    #[serde(default = "default_chart_height")]
    pub height: u32,
}

fn parse_duration<'de, D>(deser: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = Duration;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(
                formatter,
                "a non-negative number of seconds, or a string formatted duration"
            )
        }

        fn visit_str<E>(self, _s: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            // TODO: parse various strings
            // - 5s
            // - 100ms
            // - 1s500ms
            Err(serde::de::Error::custom(
                "todo: parsing string durations is not currently implemented",
            ))
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Duration::from_secs(v))
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if v < 0 {
                Err(serde::de::Error::invalid_value(
                    serde::de::Unexpected::Signed(v),
                    &self,
                ))
            } else {
                Ok(Duration::from_secs(v as u64))
            }
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if let Ok(d) = Duration::try_from_secs_f64(v) {
                Ok(d)
            } else {
                Err(serde::de::Error::invalid_value(
                    serde::de::Unexpected::Float(v),
                    &self,
                ))
            }
        }
    }

    deser.deserialize_any(V)
}

fn read_config_contents(path: &Path) -> Result<String> {
    const MAX_CONFIG_FILE_SIZE: u64 = 50 * 1024 * 1024;
    match crate::read::read_to_string_with_limit(path, MAX_CONFIG_FILE_SIZE) {
        Ok(buffer) => Ok(buffer),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                // If the config file does not exist, treat it like an empty file
                Ok("".to_string())
            } else if err.kind() == std::io::ErrorKind::InvalidData {
                return Err(anyhow!("config file {} is too large", path.display()));
            } else {
                return Err(err.into());
            }
        }
    }
}
