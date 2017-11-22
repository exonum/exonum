use slog_term::{TermDecorator, FullFormat, TermDecoratorBuilder, CompactFormat};
use slog::{Never, Drain, FnValue, PushFnValue, Record};
use slog_json::Json;
use std::time::SystemTime;
use std::fs::File;
use std::io::{self, Write};

use super::config::{LoggerOption, OutputConfig, ColorConfig, TimestampConfig, FormatConfig};

lazy_static! {
    static ref INIT_TIMER: SystemTime = SystemTime::now();
}

pub(crate) fn build_drain(opts: &LoggerOption) -> Box<Drain<Ok = (), Err = Never> + Send> {
    match *opts {
        LoggerOption::Stdout(ref o) => {
            match o.format {
                FormatConfig::Json => make_json(::std::io::stdout(), o),
                FormatConfig::Full => full_terminal(TermDecorator::new().stdout(), o),
                FormatConfig::Compact => compact_terminal(TermDecorator::new().stdout(), o),
            }
        }
        LoggerOption::Stderr(ref o) => {
            match o.format {
                FormatConfig::Json => make_json(::std::io::stderr(), o),
                FormatConfig::Full => full_terminal(TermDecorator::new().stderr(), o),
                FormatConfig::Compact => compact_terminal(TermDecorator::new().stderr(), o),
            }
        }
        LoggerOption::File(ref f, ref o) => {
            make_json(File::create(f).expect("Couldn't open file."), o)
        }
    }
}

fn color_terminal(builder: TermDecoratorBuilder, opts: &OutputConfig) -> TermDecoratorBuilder {
    match opts.color {
        ColorConfig::Always => builder.force_color(),
        ColorConfig::Auto => builder,
        ColorConfig::None => builder.force_plain(),
    }
}

fn full_terminal(
    builder: TermDecoratorBuilder,
    opts: &OutputConfig,
) -> Box<Drain<Ok = (), Err = Never> + Send> {
    let format = FullFormat::new(color_terminal(builder, opts).build());
    let format = match opts.timestamp {
        TimestampConfig::Utc => format.use_utc_timestamp(),
        TimestampConfig::None => format.use_custom_timestamp(nop_timestamp),
        TimestampConfig::Local => format.use_local_timestamp(),
        TimestampConfig::Ticks => format.use_custom_timestamp(tick_timestamp),
    };
    Box::new(format.build().filter_level(opts.level).fuse())
}

fn compact_terminal(
    builder: TermDecoratorBuilder,
    opts: &OutputConfig,
) -> Box<Drain<Ok = (), Err = Never> + Send> {
    let format = CompactFormat::new(color_terminal(builder, opts).build());
    let format = match opts.timestamp {
        TimestampConfig::Utc => format.use_utc_timestamp(),
        TimestampConfig::None => format.use_custom_timestamp(nop_timestamp),
        TimestampConfig::Local => format.use_local_timestamp(),
        TimestampConfig::Ticks => format.use_custom_timestamp(tick_timestamp),
    };
    Box::new(format.build().filter_level(opts.level).fuse())
}

fn make_json<W: io::Write + Send + 'static>(
    w: W,
    opts: &OutputConfig,
) -> Box<Drain<Ok = (), Err = Never> + Send> {

    let json = Json::new(w);
    let json = match opts.timestamp {
        TimestampConfig::None => json,
        TimestampConfig::Utc => {
            json.add_key_value(o!("ts" => FnValue(move |_| {
                                        timestamp_term_to_json(::slog_term::timestamp_utc)
                                                        })))
        }
        TimestampConfig::Local => {
            json.add_key_value(o!("ts" => FnValue(move |_| {
                                    timestamp_term_to_json(::slog_term::timestamp_local)
                                                        })))
        }
        TimestampConfig::Ticks => {
            json.add_key_value(o!("ts" => FnValue(move |_| {
                                                            timestamp_term_to_json(tick_timestamp)
                                                        })))
        }
    };
    let json = json.add_key_value(o!(
            "msg" => PushFnValue(move |record : &Record, ser| {
                ser.emit(record.msg())
            }),
            "level" => FnValue(move |rinfo : &Record| {
                rinfo.level().as_str()
            }),
            )).set_newlines(true);
    Box::new(json.build().filter_level(opts.level).fuse())
}

// compatibility layer, for printing time in same format as terminal prints
fn timestamp_term_to_json(f: fn(&mut Write) -> ::std::io::Result<()>) -> String {
    let mut v = Vec::new();
    drop(f(&mut v));
    String::from_utf8(v).unwrap_or_else(|_| "error".to_owned())
}

fn tick_timestamp(w: &mut Write) -> ::std::io::Result<()> {
    match INIT_TIMER.elapsed() {
        Ok(d) => {
            w.write_fmt(format_args!(
                "{}.{:03}",
                d.as_secs(),
                d.subsec_nanos() / 1_000_000
            ))
        }
        _ => w.write_fmt(format_args!("error")),
    }
}

fn nop_timestamp(_: &mut Write) -> ::std::io::Result<()> {
    Ok(())
}
