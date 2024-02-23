# config.toml

The main config file is located at `$XDG_CONFIG_HOME/waymon/config.toml`

This file controls details like the contents of the waymon window(s), the
update interval, etc.  waymon's default behavior is to display one window per
monitor, showing the same information on all monitors.  However, you can
configure waymon to display different contents on some monitors, or to not
display a window at all on some monitors.

## Top level fields:

### mode

This controls how waymon chooses to display windows when there are multiple
monitors.  This can be set to one of the following values:

* `mirror`

  The same information will be displayed on all monitors.

* `primary`

  The window will only be displayed on one monitor.  The `monitor_rule` section
  will be used to select the primary monitor.  All rules will be processed in
  order the order they are listed in the config file, and the first monitor
  found that matches a rule will be selected as primary.  The `bar` setting in
  the monitor rules is ignored.  (This field is only used for the `per_monitor`
  mode.)

* `per_monitor`

  The configuration file will explicitly select which bar configuration to
  display on each monitor, allowing each monitor to display different
  information.  The `monitor_rule` section will be used to select the bar
  configuration to display on each monitor.

This defaults to `mirror` if not specified.

### `interval`

A float specifying the update interval, in seconds.  This controls both how
often stats are collected from the system, and how often the charts are
re-rendered in the UI.

This defaults to 1 second.

### `width`

The default width for each bar.  This defaults to 100 pixels.

### `side`

The default side for each bar.  If a bar does not specify its own side
configuration, it will use this value.  Defaults to `right`.

### `monitor_rule`

A list of rules used to match monitors.  Each rule can contain the following
fields used to match the monitor information:

* `manufacturer`
* `model`
* `connector`

Each of these fields is treated as a regular expression pattern.  A rule
matches a monitor if all of the listed fields match the monitor's information.

A monitor rule can also contain a `bar` field, which is used to select the bar
configuration for the monitor, when running in `per_monitor` mode.  This can be
set to a bar configuration name, or `none` to disable showing a window on
matching monitors.

### `bar`

This is a dictionary of bar configurations, mainly used for `per_monitor` mode.
In `mirror` and `primary` mode, only a single bar configuration is used.  This
single bar configuration may be specified in the config file as `bar.primary`

Each bar configuration entry can have the following entries:

* `width`
   The width of the bar, in pixels.  If not set, uses the default `width`
   configuration specified at the top-level of the config file.

* `side`
   The side of the screen the bar should be shown on.  If not set, uses the
   default `side` configuration specified at the top-level of the config file.

* `widget`
   A list of widgets to show in this bar

## `widget`

The top-level `widget` configuration can be used to specify the widgets for the
`primary` bar configuration.  This is just an alias for `bar.primary.widget`

It is an error to specify both a top-level `widget` list and a
`bar.primary.widget` list.

## Widget configuration

TODO: document this more

some examples:

```
[[widget]]
type = "cpu"
label = "CPU"

[[widget]]
type = "mem"
label = "Memory"

[[widget]]
type = "disk_io"
label = "SSD I/O"
disk = "nvme0n1"

[[widget]]
type = "disk_io"
label = "HD I/O"
disk = "sda"

[[widget]]
type = "net"
dev = "wlp0s20f3"
label = "Wifi"

[[widget]]
type = "net"
dev = "lxcbr0"
label = "VM Net"
```

# style.css

A CSS configuration file for GTK can be placed at
`$XDG_CONFIG_HOME/waymon/style.css`

This will be used for styling of GTK widgets, including things like background
color, margins and padding between widgets, etc.
