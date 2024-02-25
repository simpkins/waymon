pub mod cpu;
pub mod disk_io;
pub mod mem;
pub mod net;
pub mod timeseries;
pub mod pressure;

use crate::stats::AllStats;
use std::cell::RefCell;
use std::rc::Rc;

pub trait Widget {
    fn update(&mut self);

    /// Return the top-level gtk::Widget for this widget.
    ///
    /// This is called by the Bar in order to add the gtk widget to it's window.
    ///
    /// Note: this method currently returns a reference to a gtk::Box, which requires all
    /// implementers to use a gtk::Box as their top-level widget.  In practice they all use
    /// gtk::Box today, so this isn't a problem.  If we wanted to support arbitrary widgets in the
    /// future we could return a gtk::Widget instead.  This would require callers use
    /// widget.clone().into() to turn their concrete widget type into a gtk::Widget.  This isn't
    /// really a huge deal, but we avoid it for now given that everything is using gtk::Box in
    /// practice.
    fn gtk_widget<'a>(&'a self) -> &'a gtk::Box;
}

pub trait WidgetConfig {
    fn new_widget(&mut self, all_stats: &mut AllStats) -> Rc<RefCell<dyn Widget>>;
}
