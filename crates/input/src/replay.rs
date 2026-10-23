//! Strict, dependency-free, versioned synthetic fixture reader. Never records input.
use crate::{
    Buttons, Phase, PointerSample, Tool,
    lifecycle::{Output, Processor, Reason, valid_sample},
};
use std::{error::Error, fmt, str::FromStr};

pub const MAX_RECORDS: usize = 100_000;
pub const MAX_FIXTURE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Record {
    Sample(PointerSample),
    CaptureLost { time_ms: u64, id: u64 },
    Interrupt { time_ms: u64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Replay {
    records: Vec<Record>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: &'static str,
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "replay line {}: {}", self.line, self.message)
    }
}
impl Error for ParseError {}

fn number<T: FromStr>(s: &str) -> Result<T, &'static str> {
    s.parse().map_err(|_| "invalid number")
}
fn optional(s: &str) -> Result<Option<f32>, &'static str> {
    if s == "-" {
        Ok(None)
    } else {
        number(s).map(Some)
    }
}
fn boolean(s: &str) -> Result<bool, &'static str> {
    match s {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err("expected 0 or 1"),
    }
}

fn record(fields: &[&str]) -> Result<Record, &'static str> {
    match fields {
        ["capture_lost", time, id] => Ok(Record::CaptureLost {
            time_ms: number(time)?,
            id: number(id)?,
        }),
        ["interrupt", time] => Ok(Record::Interrupt {
            time_ms: number(time)?,
        }),
        [
            "sample",
            time,
            id,
            tool,
            phase,
            x,
            y,
            pressure,
            tx,
            ty,
            buttons,
            contact,
            range,
        ] => {
            let bits: u8 = number(buttons)?;
            if bits > 31 {
                return Err("unknown button bits");
            }
            let s = PointerSample {
                timestamp_ms: number(time)?,
                id: number(id)?,
                tool: match *tool {
                    "pen" => Tool::Pen,
                    "touch" => Tool::Touch,
                    "mouse" => Tool::Mouse,
                    "unknown" => Tool::Unknown,
                    _ => return Err("unknown tool"),
                },
                phase: match *phase {
                    "down" => Phase::Down,
                    "move" => Phase::Move,
                    "up" => Phase::Up,
                    "hover" => Phase::Hover,
                    "cancel" => Phase::Cancel,
                    _ => return Err("unknown phase"),
                },
                position_px: [number(x)?, number(y)?],
                pressure: optional(pressure)?,
                tilt_degrees: [optional(tx)?, optional(ty)?],
                buttons: Buttons {
                    primary: bits & 1 != 0,
                    secondary: bits & 2 != 0,
                    barrel: bits & 4 != 0,
                    eraser: bits & 8 != 0,
                    inverted: bits & 16 != 0,
                },
                in_contact: boolean(contact)?,
                in_range: boolean(range)?,
            };
            if !valid_sample(&s) {
                return Err("invalid sample values or contact flags");
            }
            Ok(Record::Sample(s))
        }
        _ => Err("unknown record or wrong field count"),
    }
}

impl FromStr for Replay {
    type Err = ParseError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() > MAX_FIXTURE_BYTES {
            return Err(ParseError {
                line: 0,
                message: "fixture byte limit exceeded",
            });
        }
        let mut header = false;
        let mut records = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let error = |message| ParseError {
                line: index + 1,
                message,
            };
            if !header {
                if line != "paint-input-replay 1" {
                    return Err(error("expected paint-input-replay 1"));
                }
                header = true;
                continue;
            }
            if records.len() == MAX_RECORDS {
                return Err(error("record limit exceeded"));
            }
            // Every known record has at most 13 fields. Bound temporary parsing
            // allocations even for maliciously long lines.
            let fields: Vec<_> = line.split_whitespace().take(14).collect();
            records.push(record(&fields).map_err(error)?);
        }
        if !header {
            return Err(ParseError {
                line: 0,
                message: "missing version header",
            });
        }
        Ok(Self { records })
    }
}

impl Replay {
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// No sleeping, wall clock, sorting, IO, or random values. File order wins for
    /// ties. EOF explicitly cancels unfinished contacts. Output retention is owned
    /// by this caller; live applications should consume Processor outputs directly.
    pub fn run(&self) -> Vec<Output> {
        let mut processor = Processor::default();
        let mut outputs = Vec::new();
        for record in &self.records {
            match *record {
                Record::Sample(sample) => outputs.extend(processor.process(sample)),
                Record::CaptureLost { time_ms, id } => {
                    outputs.push(processor.cancel(id, time_ms, Reason::CaptureLost))
                }
                Record::Interrupt { time_ms } => {
                    outputs.extend(processor.interrupt(time_ms, Reason::Interrupted))
                }
            }
        }
        outputs.extend(processor.interrupt(processor.time_ms(), Reason::EndOfReplay));
        outputs
    }
}
