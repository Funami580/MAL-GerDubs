use chrono::Local;
use env_logger::fmt::{Color, Style, StyledValue};
use env_logger::{Builder, Logger};
use log::Level;

pub fn default_logger() -> Logger {
    formatted_local_time_builder("%H:%M:%S.%3f")
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .build()
}

fn formatted_local_time_builder(fmt: &'static str) -> Builder {
    let mut builder = env_logger::Builder::new();

    builder.format(|f, record| {
        use std::io::Write;

        let mut style = f.style();
        let level = colored_level(&mut style, record.level());

        let time = Local::now().format(fmt);

        writeln!(f, "{} {} > {}", time, level, record.args(),)
    });

    builder
}

fn colored_level(style: &'_ mut Style, level: Level) -> StyledValue<'_, &'static str> {
    match level {
        Level::Trace => style.set_color(Color::Magenta).value("TRACE"),
        Level::Debug => style.set_color(Color::Blue).value("DEBUG"),
        Level::Info => style.set_color(Color::Green).value("INFO "),
        Level::Warn => style.set_color(Color::Yellow).value("WARN "),
        Level::Error => style.set_color(Color::Red).value("ERROR"),
    }
}
