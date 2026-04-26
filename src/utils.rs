use std::fmt::{Display, Formatter, Result};

#[derive(Clone, Copy)]
pub enum SizeUnit {
    Bytes,
    KiloBytes,
    MegaBytes,
    GigaBytes,
    TeraBytes, // maybee
}

impl Display for SizeUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let s = match self {
            SizeUnit::Bytes => "B",
            SizeUnit::KiloBytes => "KB",
            SizeUnit::MegaBytes => "MB",
            SizeUnit::GigaBytes => "GB",
            SizeUnit::TeraBytes => "TB",
        };
        f.write_str(s)
    }
}

const BASE: f64 = 1024.0;

pub fn format_size(mut size: f64) -> (f64, SizeUnit) {
    let units = [
        SizeUnit::Bytes,
        SizeUnit::KiloBytes,
        SizeUnit::MegaBytes,
        SizeUnit::GigaBytes,
        SizeUnit::TeraBytes,
    ];

    let mut unit = SizeUnit::Bytes;

    for next_unit in &units {
        unit = *next_unit;
        if size < BASE {
            break;
        }
        size /= BASE;
    }

    (size, unit)
}

pub fn format_size_str(size: f64) -> String {
    let (value, unit) = format_size(size);
    if value.fract() == 0.0 {
        format!("{} {}", value as u64, unit)
    } else {
        format!("{:.2} {}", value, unit)
    }
}
