use anyhow::{anyhow, Context, Result};
use gtk::pango::EllipsizeMode;
use gtk::prelude::*;
use serde::{Deserialize, Deserializer};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

#[derive(Debug, Deserialize)]
struct CpuWidgetConfig {
    label: String,
}

#[derive(Debug, Deserialize)]
struct DiskIOWidgetConfig {
    label: String,
    disk: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum WidgetConfig {
    #[serde(rename = "cpu")]
    Cpu(CpuWidgetConfig),
    #[serde(rename = "disk_io")]
    DiskIO(DiskIOWidgetConfig),
}

#[derive(Debug, Deserialize)]
struct ConfigToml {
    #[serde(default)]
    #[serde(deserialize_with = "parse_option_duration")]
    pub interval: Option<Duration>,

    #[serde(default)]
    pub widget: Vec<WidgetConfig>,
}

#[derive(Debug)]
struct Config {
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
        let config_contents = read_file_size_limited(path)?;
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

pub struct Waymon {
    config_dir: PathBuf,
    config: Config,
}

impl Waymon {
    pub fn new(config_dir: &Path) -> Result<Waymon> {
        let waymon = Waymon {
            config_dir: config_dir.to_path_buf(),
            config: Config::load(&config_dir.join("config.toml"))?,
        };
        Ok(waymon)
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

fn read_file_size_limited(path: &Path) -> Result<String> {
    match std::fs::File::open(&path) {
        Ok(f) => {
            let mut buffer = String::new();
            // Limit the read size, to avoid using an excessive amount of memory if the
            // file is huge for some reason.
            const MAX_CONFIG_FILE_SIZE: u64 = 50 * 1024 * 1024;
            let mut handle = f.take(MAX_CONFIG_FILE_SIZE);
            handle.read_to_string(&mut buffer)?;

            // If we read exactly MAX_CONFIG_FILE_SIZE, the config file may have been larger
            // and we read only truncated data.
            if buffer.len() == MAX_CONFIG_FILE_SIZE as usize {
                return Err(anyhow!("config file is too large"));
            }

            Ok(buffer)
        }
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                // If the config file does not exist, treat it like an empty file
                Ok("".to_string())
            } else {
                return Err(err.into());
            }
        }
    }
}
