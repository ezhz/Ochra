
use std::time::*;

// ------------------------------------------------------------

pub fn show_error_box<E>(error: &E, exit: bool) -> ()
where E: std::fmt::Display
{
    eprintln!("{error}");
    msgbox::create
    (
        "",
        &error.to_string(),
        msgbox::IconType::Error
    ).unwrap();
    if exit {std::process::exit(1)}
}

// ------------------------------------------------------------

pub fn spin(interval: Duration) -> ()
{
    let now = Instant::now();
    while now.elapsed() < interval {}
}
