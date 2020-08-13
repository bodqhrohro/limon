extern crate procfs;
extern crate lazy_static;
extern crate linereader;
extern crate regex;
extern crate rust_decimal;

use std::env;
use std::fs;
use std::path;
use std::io;
use std::io::{Read, Write};
use std::str::FromStr;
use std::collections::BTreeMap;

use lazy_static::lazy_static;
use linereader::LineReader;
use regex::Regex;
use rust_decimal::Decimal;

pub struct Command
{
    pub icon: char,
    pub call: fn(&[&str]) -> Option<String>,
}

const FILE_PREFIX: &str = ".limon-";

lazy_static! {
    static ref DECIMAL_0: Decimal = Decimal::from(0);
    static ref DECIMAL_1: Decimal = Decimal::from(1);
    static ref DECIMAL_100: Decimal = Decimal::from(100);
}

// prefer /run, but /tmp is fine too
lazy_static! {
    static ref TEMP_DIR: path::PathBuf = {
        Some(match env::var("XDG_RUNTIME_DIR") {
            Ok(dir) => path::PathBuf::from(dir),
            Err(_) => env::temp_dir(),
        }).expect("")
    };
}

fn persist_state(name: &str, save: &str) -> io::Result<String> {
    let mut file_path_buf = TEMP_DIR.clone();
    file_path_buf.push(FILE_PREFIX.to_owned() + name);
    let file_path = file_path_buf.as_path();

    let mut prev_state = String::new();

    // nothing bad if the file doesn't exist, just return an empty state
    let old_file = fs::File::open(file_path);
    if let Ok(mut old_file) = old_file {
        old_file.read_to_string(&mut prev_state)?;
    }

    let new_file = fs::File::create(file_path)?;
    write!(&new_file, "{}", save)?;

    Ok(prev_state)
}

fn read_u32_from_file(filename: &str) -> io::Result<u32> {
    let mut file = fs::File::open(filename)?;
    let mut contents = String::new();

    file.read_to_string(&mut contents)?;

    if contents.ends_with('\n') {
        contents.pop();
    }

    match u32::from_str(&contents) {
        Ok(number) => Ok(number),
        Err(e) => Err(io::Error::new(io::ErrorKind::InvalidInput, e))
    }
}

fn cpu_freq_icon(cpu_no: &str) -> io::Result<String> {
    let path_base = "/sys/devices/system/cpu/cpufreq/policy".to_string() + cpu_no;

    let min_freq = read_u32_from_file(&(path_base.to_string() + "/cpuinfo_min_freq"))?;
    let max_freq = read_u32_from_file(&(path_base.to_string() + "/cpuinfo_max_freq"))?;
    let cur_freq = read_u32_from_file(&(path_base + "/scaling_cur_freq"))?;

    let boundary1 = min_freq + (max_freq - min_freq) / 3;
    let boundary2 = min_freq + (max_freq - min_freq) * 2 / 3;

    if cur_freq < boundary1 {
        Ok("·".to_string())
    } else if cur_freq > boundary2 {
        Ok("⁝".to_string())
    } else {
        Ok("⁚".to_string())
    }
}

fn format_amount(mantissa: Decimal) -> String {
    let is_round = mantissa.round_dp(3).fract() == *DECIMAL_0;

    if !is_round && mantissa < *DECIMAL_1 {
        format!("{:.2}", mantissa)
    } else if !is_round && mantissa < *DECIMAL_100 {
        format!("{:.1}", mantissa)
    } else {
        format!("{:.0}", mantissa)
    }
}

lazy_static! {
    static ref BYTE_SUFFIX_MAP: BTreeMap<u64, &'static str> = {
        let mut map = BTreeMap::new();
        map.insert(10 * (1 << 10), "K");
        map.insert(10 * (1 << 20), "M");
        map.insert(10 * (1 << 30), "G");
        map.insert(10 * (1 << 40), "T");
        map.insert(10 * (1 << 50), "P");
        map
    };
}
fn format_two_amounts(a1: u64, a2: u64, separator: &str) -> String {
    let bearer = std::cmp::max(a1, a2);

    let mut bearer_ceil = &10;
    for ceil in BYTE_SUFFIX_MAP.keys() {
        // stop when this ceil would lead to a leading zero or too many digits
        if bearer < *ceil {
            break;
        }
        bearer_ceil = ceil;
    }

    let bearer_suffix = match BYTE_SUFFIX_MAP.get(bearer_ceil) {
        Some(suffix) => suffix,
        None => "B"
    };

    let bearer_ceil = Decimal::from(bearer_ceil / 10);

    format_amount(Decimal::from(a1) / bearer_ceil) + separator +
        &format_amount(Decimal::from(a2) / bearer_ceil) + bearer_suffix
}

pub const LOADAVG:Command = Command {
    icon: '',
    call: |_| {
        let la = procfs::LoadAverage::new();
        match la {
            Ok(la) => Some(format!("{:.2} {:.2}", la.one, la.five)),
            Err(_) => None,
        }
    },
};

lazy_static! {
    static ref CPU_LINE_REGEXP: Regex = Regex::new(r"^cpu(\d+) ").unwrap();
}
pub const CPU:Command = Command {
    icon: '',
    call: |_| {
        match fs::File::open("/proc/stat") {
            Ok(stat_file) => {
                let mut linereader = LineReader::new(stat_file);

                let mut cpuinfos: Vec<String> = vec![];

                match linereader.for_each(|line| {
                    if let Ok(str_line) = std::str::from_utf8(line) {
                        match CPU_LINE_REGEXP.captures(str_line) {
                            Some(caps) => {
                                let a: Vec<&str> = str_line.split(" ").collect();
                                if a.len() >= 9 {
                                    match || -> Result<(), rust_decimal::Error> {
                                        let user = Decimal::from_str(a[1])?;
                                        let nice = Decimal::from_str(a[2])?;
                                        let system = Decimal::from_str(a[3])?;
                                        let idle = Decimal::from_str(a[4])?;
                                        let iowait = Decimal::from_str(a[5])?;
                                        let irq = Decimal::from_str(a[6])?;
                                        let softirq = Decimal::from_str(a[7])?;
                                        let steal = Decimal::from_str(a[8])?;

                                        let used = user + nice + system + irq + softirq + steal;
                                        let total = used + idle + iowait;

                                        let new_state = format!("{} {}", used, total);
                                        // save anyway, display only if there was an old state
                                        match persist_state(&("old".to_owned() + a[0]), &new_state) {
                                            Ok(old_state) => {
                                                let old_state: Vec<&str> = old_state.split(" ").collect();
                                                if old_state.len() == 2 {
                                                    let cpu_no = caps.get(1).unwrap().as_str();

                                                    let old_used = Decimal::from_str(old_state[0])?;
                                                    let old_total = Decimal::from_str(old_state[1])?;
                                                    cpuinfos.push(if total > old_total {
                                                        format!(
                                                            "{}{:.0}%",
                                                            cpu_freq_icon(cpu_no).unwrap_or("".to_string()),
                                                            *DECIMAL_100 * (used - old_used) / (total - old_total)
                                                        )
                                                    } else {
                                                        "?".to_string()
                                                    });
                                                }
                                            },
                                            Err(_) => {}
                                        };

                                        Ok(())
                                    }() {
                                        Ok(_) => {},
                                        Err(_) => {},
                                    };
                                }
                            },
                            None => {}
                        }
                    }

                    Ok(true)
                }) {
                    Ok(_) => Some(cpuinfos.join(" ")),
                    Err(_) => None
                }
            },
            Err(_) => None
        }
    },
};

lazy_static! {
    static ref MEMINFO: procfs::ProcResult<procfs::Meminfo> = procfs::meminfo();
}
pub const MEM:Command = Command {
    icon: '',
    call: |_| {
        match &*MEMINFO {
            Ok(meminfo) => {
                let mem_available = meminfo.mem_available.unwrap_or(0);
                let mem_total = meminfo.mem_total;

                Some(format_two_amounts(mem_total - mem_available, mem_total, "/"))
            },
            Err(_) => None,
        }
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    // unexpected behaviour, but shoudn't fail at least
    #[test]
    fn amount_format_negative() {
        let formatted = format_amount(Decimal::new(-2368, 2));
        assert_eq!(formatted, "-23.68");
    }

    #[test]
    fn amount_format_zero() {
        let formatted = format_amount(Decimal::from(0));
        assert_eq!(formatted, "0");
    }

    #[test]
    fn amount_format_tiny() {
        let formatted = format_amount(Decimal::new(1, 3));
        assert_eq!(formatted, "0.00");
    }

    #[test]
    fn amount_format_almost_one() {
        let formatted = format_amount(Decimal::new(996, 3));
        assert_eq!(formatted, "0.99");
    }

    #[test]
    fn amount_format_one() {
        let formatted = format_amount(Decimal::from(1));
        assert_eq!(formatted, "1");
    }

    #[test]
    fn amount_format_almost_ten() {
        let formatted = format_amount(Decimal::new(975, 2));
        assert_eq!(formatted, "9.7");
    }

    #[test]
    fn amount_format_ten() {
        let formatted = format_amount(Decimal::from(10));
        assert_eq!(formatted, "10");
    }

    #[test]
    fn amount_format_ten_plus() {
        let formatted = format_amount(Decimal::new(1000001, 5));
        assert_eq!(formatted, "10");
    }

    #[test]
    fn amount_format_99() {
        let formatted = format_amount(Decimal::new(9995, 2));
        assert_eq!(formatted, "99.9");
    }

    #[test]
    fn amount_format_hundred() {
        let formatted = format_amount(Decimal::from(100));
        assert_eq!(formatted, "100");
    }

    #[test]
    fn amount_format_alot() {
        let formatted = format_amount(Decimal::new(123456, 2));
        assert_eq!(formatted, "1234");
    }

    #[test]
    fn amount_format_huge() {
        let formatted = format_amount(Decimal::new(123749089, 2));
        assert_eq!(formatted, "1237490");
    }

    #[test]
    fn two_amounts_bytes() {
        let formatted = format_two_amounts(3, 687, "/");
        assert_eq!(formatted, "3/687B");
    }

    #[test]
    fn two_amounts_zero() {
        let formatted = format_two_amounts(0, 0, ":");
        assert_eq!(formatted, "0:0B");
    }

    #[test]
    fn two_amounts_zero_of_more() {
        let formatted = format_two_amounts(0, 102938, "'");
        assert_eq!(formatted, "0'100K");
    }

    #[test]
    fn two_amounts_mega() {
        let formatted = format_two_amounts(1232899, 2389999, "=");
        assert_eq!(formatted, "1204=2333K");
    }

    #[test]
    fn two_amounts_mega_slight_asym() {
        let formatted = format_two_amounts(1232899, 23899999, "⁚");
        assert_eq!(formatted, "1.1⁚22.7M");
    }

    #[test]
    fn two_amounts_mega_very_asym() {
        let formatted = format_two_amounts(123289, 23899999, r"\");
        assert_eq!(formatted, r"0.11\22.7M");
    }

    #[test]
    fn two_amounts_mega_extreme_asym() {
        let formatted = format_two_amounts(123289, 23899999999, "O");
        assert_eq!(formatted, "0O22.2G");
    }

    #[test]
    fn two_amounts_first_larger() {
        let formatted = format_two_amounts(23899999999, 123289, "lol");
        assert_eq!(formatted, "22.2lol0G");
    }
}
