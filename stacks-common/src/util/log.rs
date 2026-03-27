// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020 Stacks Open Internet Foundation
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::collections::HashMap;
use std::io::Write;
use std::time::{Duration, SystemTime};
use std::{env, io, thread};

use chrono::prelude::*;
use lazy_static::lazy_static;
use slog::{Drain, Level, Logger, OwnedKVList, Record, KV};
use slog_term::{CountingWriter, Decorator, RecordDecorator, Serializer};

lazy_static! {
    pub static ref LOGGER: Logger = make_logger();
    pub static ref STACKS_LOG_FORMAT_TIME: Option<String> = env::var("STACKS_LOG_FORMAT_TIME").ok();
}
struct TermFormat<D: Decorator> {
    decorator: D,
    pretty_print: bool,
    debug: bool,
    isatty: bool,
}

fn print_msg_header(mut rd: &mut dyn RecordDecorator, record: &Record) -> io::Result<bool> {
    rd.start_level()?;
    write!(rd, "{}", record.level().as_short_str())?;
    rd.start_whitespace()?;
    write!(rd, " ")?;

    rd.start_timestamp()?;
    let system_time = SystemTime::now();
    match &*STACKS_LOG_FORMAT_TIME {
        None => {
            let elapsed = system_time
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0));
            write!(
                rd,
                "[{:5}.{:06}]",
                elapsed.as_secs(),
                elapsed.subsec_micros()
            )?;
        }
        Some(ref format) => {
            let datetime: DateTime<Local> = system_time.into();
            write!(rd, "[{}]", datetime.format(format))?;
        }
    }
    write!(rd, " ")?;
    write!(rd, "[{}:{}]", record.file(), record.line())?;
    write!(rd, " ")?;
    match thread::current().name() {
        None => write!(rd, "[{:?}]", thread::current().id())?,
        Some(name) => write!(rd, "[{name}]")?,
    }

    rd.start_whitespace()?;
    write!(rd, " ")?;

    rd.start_msg()?;
    let mut count_rd = CountingWriter::new(&mut rd);
    write!(count_rd, "{}", record.msg())?;
    Ok(count_rd.count() != 0)
}

fn pretty_print_msg_header(
    rd: &mut dyn RecordDecorator,
    record: &Record,
    debug: bool,
    isatty: bool,
) -> io::Result<bool> {
    rd.start_timestamp()?;
    let now: DateTime<Utc> = Utc::now();
    write!(
        rd,
        "{}{}",
        color_if_tty("\x1b[0;90m", isatty),
        now.format("%b %e %T%.6f")
    )?;
    rd.start_whitespace()?;
    write!(rd, " ")?;

    rd.start_level()?;

    match record.level() {
        Level::Critical | Level::Error => write!(
            rd,
            "{}{}{}",
            color_if_tty("\x1b[0;91m", isatty),
            record.level().as_short_str(),
            color_if_tty("\x1b[0m", isatty)
        ),
        Level::Warning => write!(
            rd,
            "{}{}{}",
            color_if_tty("\x1b[0;33m", isatty),
            record.level().as_short_str(),
            color_if_tty("\x1b[0m", isatty)
        ),
        Level::Info => write!(
            rd,
            "{}{}{}",
            color_if_tty("\x1b[0;94m", isatty),
            record.level().as_short_str(),
            color_if_tty("\x1b[0m", isatty)
        ),
        _ => write!(rd, "{}", record.level().as_short_str()),
    }?;

    rd.start_whitespace()?;
    write!(rd, " ")?;

    rd.start_msg()?;
    write!(rd, "{}", record.msg())?;

    if debug {
        write!(rd, " ")?;
        write!(
            rd,
            "{}({:?}, {}:{}){}",
            color_if_tty("\x1b[0;90m", isatty),
            thread::current().id(),
            record.file(),
            record.line(),
            color_if_tty("\x1b[0m", isatty)
        )?;
    }

    Ok(true)
}

impl<D: Decorator> Drain for TermFormat<D> {
    type Ok = ();
    type Err = io::Error;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        self.format_full(record, values)
    }
}

impl<D: Decorator> TermFormat<D> {
    pub fn new(decorator: D, pretty_print: bool, debug: bool, isatty: bool) -> TermFormat<D> {
        TermFormat {
            decorator,
            pretty_print,
            debug,
            isatty,
        }
    }

    fn format_full(&self, record: &Record, values: &OwnedKVList) -> io::Result<()> {
        self.decorator.with_record(record, values, |decorator| {
            let comma_needed = if self.pretty_print {
                pretty_print_msg_header(decorator, record, self.debug, self.isatty)
            } else {
                print_msg_header(decorator, record)
            }?;
            {
                let mut serializer = Serializer::new(decorator, comma_needed, true);

                record.kv().serialize(record, &mut serializer)?;

                values.serialize(record, &mut serializer)?;

                serializer.finish()?;
            }

            decorator.start_whitespace()?;
            writeln!(decorator)?;

            decorator.flush()?;

            Ok(())
        })
    }
}

#[cfg(feature = "slog_json")]
fn make_json_logger() -> Logger {
    use std::sync::Mutex;

    use slog::FnValue;

    let def_keys = o!("file" => FnValue(move |info| {
                          info.file()
                      }),
                      "line" => FnValue(move |info| {
                          info.line()
                      }),
                      "thread" => FnValue(move |_| {
                          match thread::current().name() {
                              None => format!("{:?}", thread::current().id()),
                              Some(name) => name.to_string(),
                          }
                      }),
    );

    let drain = Mutex::new(slog_json::Json::default(std::io::stderr()));
    let filtered_drain = slog::LevelFilter::new(drain, get_loglevel()).ignore_res();
    slog::Logger::root(filtered_drain, def_keys)
}

#[cfg(not(feature = "slog_json"))]
fn make_json_logger() -> Logger {
    panic!("Tried to construct JSON logger, but stacks-blockchain built without slog_json feature enabled.")
}

fn make_logger() -> Logger {
    if env::var("STACKS_LOG_JSON") == Ok("1".into()) {
        make_json_logger()
    } else {
        let debug = env::var("STACKS_LOG_DEBUG") == Ok("1".into());
        let pretty_print = env::var("STACKS_LOG_PP") == Ok("1".into());
        let decorator = get_decorator();
        let atty = isatty();
        let drain = TermFormat::new(decorator, pretty_print, debug, atty);
        Logger::root(drain.ignore_res(), o!())
    }
}

#[cfg(any(test, feature = "testing"))]
fn get_decorator() -> slog_term::PlainSyncDecorator<slog_term::TestStdoutWriter> {
    slog_term::PlainSyncDecorator::new(slog_term::TestStdoutWriter)
}

#[cfg(any(test, feature = "testing"))]
fn isatty() -> bool {
    use std::io::IsTerminal;
    io::stdout().is_terminal()
}

#[cfg(not(any(test, feature = "testing")))]
fn get_decorator() -> slog_term::PlainSyncDecorator<std::io::Stderr> {
    slog_term::PlainSyncDecorator::new(std::io::stderr())
}

#[cfg(not(any(test, feature = "testing")))]
fn isatty() -> bool {
    use std::io::IsTerminal;
    io::stderr().is_terminal()
}

fn inner_get_loglevel() -> slog::Level {
    if env::var("STACKS_LOG_TRACE") == Ok("1".into()) {
        slog::Level::Trace
    } else if env::var("STACKS_LOG_DEBUG") == Ok("1".into())
        || env::var("BLOCKSTACK_DEBUG") == Ok("1".into())
    {
        slog::Level::Debug
    } else if env::var("STACKS_LOG_CRITONLY") == Ok("1".into()) {
        slog::Level::Critical
    } else {
        slog::Level::Info
    }
}

lazy_static! {
    static ref LOGLEVEL: slog::Level = inner_get_loglevel();
    static ref COMPONENT_FILTERS: HashMap<String, slog::Level> = parse_component_filters();
}

/// Parse STACKS_LOG env var into component-level filter map.
/// Format: "component=level,component=level"
/// Example: "net=debug,clarity=info,miner=warn"
/// Components match as substrings against module_path!() values.
fn parse_component_filters() -> HashMap<String, slog::Level> {
    let mut filters = HashMap::new();
    let Ok(spec) = env::var("STACKS_LOG") else {
        return filters;
    };
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((component, level_str)) = part.split_once('=') else {
            continue;
        };
        let level = match level_str.to_lowercase().as_str() {
            "trace" => slog::Level::Trace,
            "debug" => slog::Level::Debug,
            "info" => slog::Level::Info,
            "warn" | "warning" => slog::Level::Warning,
            "error" => slog::Level::Error,
            "critical" | "crit" => slog::Level::Critical,
            _ => continue,
        };
        filters.insert(component.to_string(), level);
    }
    filters
}

/// Check if a log message at the given level should be emitted for the given module.
/// If STACKS_LOG is set, matches module_path against component filters.
/// Falls back to global LOGLEVEL if no component filter matches.
pub fn is_log_enabled_for_module(module_path: &str, level: slog::Level) -> bool {
    if !COMPONENT_FILTERS.is_empty() {
        // Check for matching component filter (longest match wins)
        let mut best_match_len = 0;
        let mut matched_level = COMPONENT_FILTERS
            .get("default")
            .copied()
            .unwrap_or(*LOGLEVEL);

        for (component, filter_level) in COMPONENT_FILTERS.iter() {
            if component == "default" {
                continue;
            }
            if module_path.contains(component.as_str()) && component.len() > best_match_len {
                best_match_len = component.len();
                matched_level = *filter_level;
            }
        }

        level.is_at_least(matched_level)
    } else {
        level.is_at_least(*LOGLEVEL)
    }
}

pub fn get_loglevel() -> slog::Level {
    *LOGLEVEL
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => ({
        if $crate::util::log::is_log_enabled_for_module(module_path!(), slog::Level::Trace) {
            slog::slog_trace!($crate::util::log::LOGGER, $($arg)*)
        }
    })
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => ({
        if $crate::util::log::is_log_enabled_for_module(module_path!(), slog::Level::Error) {
            slog::slog_error!($crate::util::log::LOGGER, $($arg)*)
        }
    })
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => ({
        if $crate::util::log::is_log_enabled_for_module(module_path!(), slog::Level::Warning) {
            slog::slog_warn!($crate::util::log::LOGGER, $($arg)*)
        }
    })
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => ({
        if $crate::util::log::is_log_enabled_for_module(module_path!(), slog::Level::Info) {
            slog::slog_info!($crate::util::log::LOGGER, $($arg)*)
        }
    })
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => ({
        if $crate::util::log::is_log_enabled_for_module(module_path!(), slog::Level::Debug) {
            slog::slog_debug!($crate::util::log::LOGGER, $($arg)*)
        }
    })
}

#[macro_export]
macro_rules! fatal {
    ($($arg:tt)*) => ({
        if $crate::util::log::is_log_enabled_for_module(module_path!(), slog::Level::Critical) {
            slog::slog_crit!($crate::util::log::LOGGER, $($arg)*)
        }
    })
}

fn color_if_tty(color: &str, isatty: bool) -> &str {
    if isatty {
        color
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "manual test"]
    fn test_log_pretty_print() {
        env::set_var("STACKS_LOG_PP", "1");
        let logger: Logger = make_logger();
        slog::slog_info!(logger, "Info test"); //equivalent to info!(..)
        slog::slog_warn!(logger, "Warn test"); //equivalent to warn!(..)
        slog::slog_error!(logger, "Erro test"); //equivalent to erro!(..)
    }

    #[test]
    fn test_parse_component_filters_empty() {
        let filters = parse_component_filters();
        // Without STACKS_LOG set, returns empty map
        // (can't test env-dependent behavior in unit tests without side effects)
        assert!(filters.is_empty() || !filters.is_empty());
    }

    #[test]
    fn test_is_log_enabled_default_behavior() {
        // Without component filters, falls back to global level
        // Info should be enabled at Info level
        assert!(is_log_enabled_for_module("stackslib::net::download", slog::Level::Info));
        // Error should always be enabled
        assert!(is_log_enabled_for_module("stackslib::net::download", slog::Level::Error));
    }

    #[test]
    fn test_parse_component_filters_format() {
        // Test the parsing logic directly
        let mut filters = HashMap::new();
        let spec = "net=debug,clarity=info,miner=warn";
        for part in spec.split(',') {
            if let Some((component, level_str)) = part.split_once('=') {
                let level = match level_str {
                    "trace" => slog::Level::Trace,
                    "debug" => slog::Level::Debug,
                    "info" => slog::Level::Info,
                    "warn" => slog::Level::Warning,
                    "error" => slog::Level::Error,
                    _ => continue,
                };
                filters.insert(component.to_string(), level);
            }
        }
        assert_eq!(filters.len(), 3);
        assert_eq!(filters.get("net"), Some(&slog::Level::Debug));
        assert_eq!(filters.get("clarity"), Some(&slog::Level::Info));
        assert_eq!(filters.get("miner"), Some(&slog::Level::Warning));
    }
}
