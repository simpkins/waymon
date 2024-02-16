fn format_f64_helper(value: f64, denom: f64, suffix: &str) -> String {
    let x = value / denom;
    if x < 10.0 {
        format!("{:.2}{}", x, suffix)
    } else if x < 100.0 {
        format!("{:.1}{}", x, suffix)
    } else {
        format!("{:.0}{}", x, suffix)
    }
}

pub fn humanify_f64(value: f64) -> String {
    if value < 1000.0 {
        format!("{}B", value as u64)
    } else if value < 1_000_000.0 {
        format_f64_helper(value, 1000.0, "KB")
    } else if value < 1_000_000_000.0 {
        format_f64_helper(value, 1_000_000.0, "MB")
    } else if value < 1_000_000_000_000.0 {
        format_f64_helper(value, 1_000_000_000.0, "GB")
    } else {
        format_f64_helper(value, 1_000_000_000_000.0, "TB")
    }
}
