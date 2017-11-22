use serde::{Serialize, Deserialize, Serializer, Deserializer};

use std::fmt::{self, Display};
use std::str::FromStr;
use std::collections::BTreeMap;

/// `LoggerConfig` structure that collect array of possible logger outputs.
/// With this config, one can initialize logger with multiple output targets.
///
/// Full template of possible configuration
///
/// ```toml
///
/// [logger.stdout]
/// format = "json" | "compact" | "full"
/// color = "always" | "auto" | "none"
/// timestamp = "utc" | "local" | "none" | "ticks"
/// level = "crit" | "error" | "warn" | "info" | "debug" | "trace" | "off" | "env"
///
/// [logger.stderr]
/// format = "json" | "compact" | "full"
/// color = "always" | "auto" | "none"
/// timestamp = "utc" | "local" | "none" | "ticks"
/// level = "crit" | "error" | "warn" | "info" | "debug" | "trace" | "off" | "env"
///
/// [logger."/path/to/file.log"]
/// format = "json"
// TODO: add support terminal output to file
// | "compact" | "full"
// color = "always" | "auto" | "none"
/// timestamp = "utc" | "local" | "none" | "ticks"
/// level = "crit" | "error" | "warn" | "info" | "debug" | "trace" | "off" | "env"
/// ```
///
/// Where `logger` is a root section in config,
/// `stdout` / `stderr` - terminal standard out, and standard error outputs;
/// `"/path/to/file.log"` - file output section.
///
/// In each section, one can set some of variables
///
/// * `format` - is possible output format, could be one of
///
///   * `json` - output in json, important for elasticsearch and other programming analysis;
///
///   * `compact` - plain text in compact mode, each entry in log grouped by context;
///
///   * `full` - plain text in full mode, no grouping applied.
///
/// * `color` - mark log levels with color, usable only for `compact` and `full` output `format`
///
///   * `none` - never color output;
///
///   * `auto` - try to detect automatically if terminal is support color output,
///                  useful when program often pipe output;
///
///   * `always` - always print color output, even when terminal is not support this feature.
///
/// * `timestamp` - print timestamp in output with event.
///
///   * `utc` - print timestamp in UTC standard;
///
///   * `local` - print timestamp using local timezone;
///
///   * `ticks` - print timestamp in microseconds since program start;
///
///   * `none` - don't print timestamp.
///
/// * `level` - minimum level, that will be printed.
///     (beware: slog can remove some of levels at compile time)
///
///   * `off` - disable logging into output
///
///   * `crit` / `error` / `warn` / `info` / `debug` / `trace`  - set's minimum log level.
///
///   * `env` - use `RUST_LOG` - environment variable
///
/// Note: file output currently support only json.
///

use super::{MultipleDrain, builder};

#[derive(Debug, Clone)]
pub struct LoggerConfig {
    loggers: Vec<LoggerOption>,
}

impl LoggerConfig {
    pub(crate) fn into_multi_logger(self) -> MultipleDrain {
        self.loggers
            .into_iter()
            .map(|v| builder::build_drain(&v))
            .collect()
    }
}

impl Default for LoggerConfig {
    fn default() -> LoggerConfig {
        let cfg = OutputConfig {
            format: FormatConfig::Full,
            color: ColorConfig::Auto,
            timestamp: TimestampConfig::Local,
            level: ::slog::Level::Info,
        };
        LoggerConfig { loggers: vec![LoggerOption::Stderr(cfg)] }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ColorConfig {
    Always,
    Auto,
    None,
}

impl Display for ColorConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            ColorConfig::Always => "always",
            ColorConfig::Auto => "auto",
            ColorConfig::None => "none",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for ColorConfig {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = match s {
            "always" => ColorConfig::Always,
            "auto" => ColorConfig::Auto,
            "none" => ColorConfig::None,
            _ => return Err(()),
        };
        Ok(val)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TimestampConfig {
    Utc,
    None,
    Local,
    Ticks,
}

impl Display for TimestampConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            TimestampConfig::Utc => "utc",
            TimestampConfig::Local => "local",
            TimestampConfig::Ticks => "ticks",
            TimestampConfig::None => "none",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for TimestampConfig {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = match s {
            "utc" => TimestampConfig::Utc,
            "local" => TimestampConfig::Local,
            "ticks" => TimestampConfig::Ticks,
            "none" => TimestampConfig::None,
            _ => return Err(()),
        };
        Ok(val)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormatConfig {
    Json,
    Compact,
    Full,
}

impl Display for FormatConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            FormatConfig::Json => "json",
            FormatConfig::Compact => "compact",
            FormatConfig::Full => "full",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for FormatConfig {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = match s {
            "json" => FormatConfig::Json,
            "compact" => FormatConfig::Compact,
            "full" => FormatConfig::Full,
            _ => return Err(()),
        };
        Ok(val)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct OutputConfig {
    pub format: FormatConfig,
    pub color: ColorConfig,
    pub timestamp: TimestampConfig,
    pub level: ::slog::Level,
}

impl OutputConfig {
    fn to_map(&self) -> BTreeMap<&'static str, String> {
        let mut map = BTreeMap::new();
        map.insert("format", self.format.to_string());
        if self.format != FormatConfig::Json {
            map.insert("color", self.color.to_string());
        }
        map.insert("timestamp", self.timestamp.to_string());
        map.insert("level", self.level.to_string());
        map
    }
    fn from_map(map: BTreeMap<String, String>) -> OutputConfig {
        OutputConfig {
            format: map.get("format")
                .map(|s| s.parse())
                .unwrap_or(Ok(FormatConfig::Compact))
                .unwrap_or(FormatConfig::Compact),
            color: map.get("color")
                .map(|s| s.parse())
                .unwrap_or(Ok(ColorConfig::Auto))
                .unwrap_or(ColorConfig::Auto),
            timestamp: map.get("timestamp")
                .map(|s| s.parse())
                .unwrap_or(Ok(TimestampConfig::Local))
                .unwrap_or(TimestampConfig::Local),
            level: map.get("level")
                .map(|s| s.parse())
                .unwrap_or(Ok(::slog::Level::Info))
                .unwrap_or(::slog::Level::Info),
        }
    }
}

impl Serialize for OutputConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let map = self.to_map();
        map.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OutputConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map: BTreeMap<String, String> = Deserialize::deserialize(deserializer)?;
        Ok(Self::from_map(map))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum LoggerOption {
    Stdout(OutputConfig),
    Stderr(OutputConfig),
    File(String, OutputConfig),
}

impl Serialize for LoggerConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let map: BTreeMap<String, BTreeMap<&'static str, String>> = self.loggers
            .iter()
            .map(|i| {
                let (n, o) = match *i {
                    LoggerOption::Stdout(ref o) => ("stdout".to_string(), o),
                    LoggerOption::Stderr(ref o) => ("stderr".to_string(), o),
                    LoggerOption::File(ref name, ref o) => (name.to_string(), o),
                };
                (n, o.to_map())
            })
            .collect();
        map.serialize(serializer)
    }
}


impl<'de> Deserialize<'de> for LoggerConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map: BTreeMap<String, BTreeMap<String, String>> =
            Deserialize::deserialize(deserializer)?;
        let loggers = map.into_iter()
            .map(|(name, config)| match name.as_str() {
                "stdout" => LoggerOption::Stdout(OutputConfig::from_map(config)),
                "stderr" => LoggerOption::Stderr(OutputConfig::from_map(config)),
                _ => LoggerOption::File(name, OutputConfig::from_map(config)),
            })
            .collect();
        Ok(LoggerConfig { loggers })
    }
}
