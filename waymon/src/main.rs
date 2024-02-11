use anyhow::anyhow;
use clap::{Parser, ValueHint};
use gtk::prelude::*;
use gtk::{glib, Orientation, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::ffi::OsString;
use std::path::PathBuf;

mod waymon;

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

    let mut waymon = match waymon::Waymon::new(&config_dir) {
        Ok(waymon) => waymon,
        Err(err) => {
            eprintln!("initialization error: {:#}", err);
            return Err(anyhow!("initialization error: {:#}", err));
        }
    };

    // I'm manually calling gtk::init and driving the glib main loop here, rather than using
    // gtk::Application.  I don't really want the gtk Application's handling of application
    // uniqueness or it's command line argument parsing an file open semantics.
    gtk::init()?;
    init_window(&mut waymon);
    let main_loop = glib::MainLoop::new(None, false);
    main_loop.run();
    Ok(())
}

fn report_css_parsing_error(
    _css: &gtk::CssProvider,
    section: &gtk::CssSection,
    error: &glib::Error,
) {
    eprintln!("CSS parsing error at {}: {}\n", section.to_str(), error);
}

fn on_tick() -> glib::ControlFlow {
    println!("tick!");
    glib::ControlFlow::Continue
}

fn init_window(waymon: &mut waymon::Waymon) {
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
    css.load_from_path(waymon.css_path());
    gtk::style_context_add_provider_for_display(
        &WidgetExt::display(&window),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    waymon.add_widgets(&box_widget);

    // waymon->update();
    glib::timeout_add(waymon.interval(), on_tick);

    // Present window
    window.present();
}
