pub fn humanify_f64(value: f64) -> String {
    if value < 1000.0 {
        return format!("{}B", value as u64);
    }
    if value < 1_000_000.0 {
        return format!("{}KB", (value as u64) / 1000);
    }
    if value < 1_000_000_000.0 {
        return format!("{}MB", (value as u64) / 1_000_000);
    }
    if value < 1_000_000_000_000.0 {
        return format!("{}GB", (value as u64) / 1_000_000_000);
    }
    return format!("{}TB", (value as u64) / 1_000_000_000_000);
}
