use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Deserializer};
use std::path::Path;
use std::time::Duration;

#[derive(Debug)]
pub struct Config {
    pub interval: Duration,
    pub widgets: Vec<WidgetConfig>,
}

impl Config {
    pub fn new() -> Config {
        Config {
            interval: Duration::from_secs(1),
            widgets: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Config> {
        let config_contents = read_config_contents(path)?;
        let config_toml: ConfigToml =
            toml::from_str(&config_contents).with_context(|| format!("{}", path.display()))?;
        let mut cfg = Self::new();
        cfg.update_from_toml(config_toml);
        Ok(cfg)
    }

    fn update_from_toml(&mut self, data: ConfigToml) {
        if let Some(i) = data.interval {
            self.interval = i;
        }
        eprintln!("testing: {:?}", data.widget);
        self.widgets = data.widget;
    }
}

#[derive(Debug, Deserialize)]
struct ConfigToml {
    #[serde(default)]
    #[serde(deserialize_with = "parse_option_duration")]
    pub interval: Option<Duration>,

    #[serde(default)]
    pub widget: Vec<WidgetConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum WidgetConfig {
    #[serde(rename = "cpu")]
    Cpu(CpuWidgetConfig),
    #[serde(rename = "disk_io")]
    DiskIO(DiskIOWidgetConfig),
}

#[derive(Debug, Deserialize)]
pub struct CpuWidgetConfig {
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct DiskIOWidgetConfig {
    pub label: String,
    pub disk: String,
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

fn parse_option_duration<'de, D>(deser: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Some(parse_duration(deser)?))
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
