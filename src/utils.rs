pub fn notice(message: &str) {
    eprintln!("\x1b[90mNOTICE: {}\x1b[0m", message);
}
