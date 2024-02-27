# waymon

A system monitor bar for wayland.

This aims to be something like [gkrellm](http://gkrellm.srcbox.net/) or
[conky](https://github.com/brndnmtthws/conky), but with decent multi-monitor
support for wayland.  It uses the
[layer-shell](https://wayland.app/protocols/wlr-layer-shell-unstable-v1)
protocol to anchor to a specific side of the screen, and can be configured to
show on all monitors, only one primary monitor, or to use different
configurations on different monitors.

This code is pretty half-baked at the moment, but has some basic functionality
for showing charts of CPU usage, disk and network I/O, and memory usage.

[![screenshot](doc/screenshot.png)](doc/screenshot.png)
