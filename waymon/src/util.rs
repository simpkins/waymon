fn format_f64_helper(value: f64, denom: f64, suffix: &str, sigfigs: u32) -> String {
    let x = value / denom;
    if sigfigs == 3 {
        if x < 10.0 {
            format!("{:.2}{}", x, suffix)
        } else if x < 100.0 {
            format!("{:.1}{}", x, suffix)
        } else {
            format!("{:.0}{}", x, suffix)
        }
    } else if sigfigs == 2 {
        if x < 10.0 {
            format!("{:.1}{}", x, suffix)
        } else {
            format!("{:.0}{}", x, suffix)
        }
    } else if sigfigs == 1 {
        format!("{:.0}{}", x, suffix)
    } else {
        // TODO: we currently don't handle higher values
        format_f64_helper(value, denom, suffix, 3)
    }
}

pub fn humanify_f64(value: f64, sigfigs: u32) -> String {
    if value < 1000.0 {
        format!("{}B", value as u64)
    } else if value < 1_000_000.0 {
        format_f64_helper(value, 1000.0, "KB", sigfigs)
    } else if value < 1_000_000_000.0 {
        format_f64_helper(value, 1_000_000.0, "MB", sigfigs)
    } else if value < 1_000_000_000_000.0 {
        format_f64_helper(value, 1_000_000_000.0, "GB", sigfigs)
    } else {
        format_f64_helper(value, 1_000_000_000_000.0, "TB", sigfigs)
    }
}
