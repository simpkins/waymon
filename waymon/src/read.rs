use std::io::Read;
use std::path::Path;

// Like read_to_string(), but with a maximum limit to avoid allocating a huge amount
// of memory if the file happens to be very large.
pub fn read_to_string_with_limit(path: &Path, max_size: u64) -> Result<String, std::io::Error> {
    let f = std::fs::File::open(&path)?;
    // TODO: we possibly could stat() the file here to check if it appears too big or not.
    // The file size may change as we are reading it, but if we know it starts out too big
    // we can go ahead and fail early.
    let mut buffer = String::new();
    // Try to read max_size + 1, so we can tell if we read too much or not.
    let mut handle = f.take(max_size + 1);
    handle.read_to_string(&mut buffer)?;
    if buffer.len() > max_size as usize {
        // We could use ErrorKind::FileTooLarge once that code makes it into stable
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file is too large",
        ));
    }

    Ok(buffer)
}
