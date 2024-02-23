use anyhow::anyhow;
use clap::{Parser, ValueHint};
use gtk::glib;
use std::ffi::OsString;
use std::path::PathBuf;

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
}

fn main() -> anyhow::Result<()> {
    let opts = Opt::parse();
    let config_dir = if let Some(x) = &opts.config_dir {
        PathBuf::from(x)
    } else if let Some(x) = dirs::config_dir() {
        x.join("waymon")
    } else {
        eprintln!("unable to determine config directory");
        return Err(anyhow!("unable to determine config directory"));
    };

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
