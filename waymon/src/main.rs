use anyhow::anyhow;
use clap::{Parser, ValueHint};
use gtk::glib;
use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::prelude::*;

mod bar;
mod collectors;
mod config;
mod read;
mod stats;
mod util;
mod waymon;
mod widgets;

#[derive(Debug, Parser)]
#[command(about = "System monitor for wayland")]
struct Opt {
    #[arg(long, value_parser, value_hint=ValueHint::DirPath)]
    config_dir: Option<OsString>,
    #[arg(short, long)]
    logging: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let opts = Opt::parse();
    let config_dir = if let Some(x) = &opts.config_dir {
        PathBuf::from(x)
    } else if let Some(x) = dirs::config_dir() {
        x.join("waymon")
    } else {
        return Err(anyhow!("unable to determine config directory"));
    };

    init_logging(opts.logging.as_deref())?;

    // I'm manually calling gtk::init and driving the glib main loop here, rather than using
    // gtk::Application.  I don't really want the gtk Application's handling of application
    // uniqueness or it's command line argument parsing.
    gtk::init()?;

    let waymon = match waymon::WaymonState::new(&config_dir) {
        Ok(waymon) => waymon,
        Err(err) => {
            return Err(anyhow!("initialization error: {:#}", err));
        }
    };

    waymon.start();
    let main_loop = glib::MainLoop::new(None, false);
    main_loop.run();
    Ok(())
}

fn init_logging(log_config: Option<&str>) -> anyhow::Result<()> {
    // Configure logging.
    // Rather than using the RUST_LOG environment variable, this uses an explicit --logging command
    // line argument, and explicitly fails if there is a parse error in the settings.
    let log_filter = if let Some(config_str) = log_config {
        Targets::from_str(&config_str)?
    } else {
        // Show warning and above by default
        Targets::new().with_default(LevelFilter::WARN)
    };
    let log_subscriber = Subscriber::builder()
        .with_max_level(LevelFilter::TRACE)
        .finish();
    log_subscriber.with(log_filter).try_init()?;
    Ok(())
}
