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

lazy_static! {
    static ref CPUFREQ_BOUNDARY_1: f32 = 1100.0;
    static ref CPUFREQ_BOUNDARY_2: f32 = 1375.0;
}
fn cpu_freq_icon(cpu_no: usize) -> procfs::ProcResult<String> {
    let cpuinfo = procfs::cpuinfo()?;
    match cpuinfo.get_info(cpu_no) {
        Some(fields) => match fields.get("cpu MHz") {
            Some(freq) => {
                match freq.parse::<f32>() {
                    Ok(freq) if freq < *CPUFREQ_BOUNDARY_1 => Ok("·".to_string()),
                    Ok(freq) if freq > *CPUFREQ_BOUNDARY_2 => Ok("⁝".to_string()),
                    Ok(_) => Ok("⁚".to_string()),
                    Err(_) => Err(procfs::ProcError::Other("Malformed CPU frequency".to_string()))
                }
            },
            None => Err(procfs::ProcError::Other("CPU does not report its frequency".to_string()))
        },
        None => Err(procfs::ProcError::Other("No such CPU".to_string()))
    }
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
    static ref DECIMAL_100: Decimal = Decimal::new(100, 0);
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
                                                    let cpu_no = caps.get(1).unwrap().as_str().parse::<usize>().unwrap_or(0);

                                                    let old_used = Decimal::from_str(old_state[0])?;
                                                    let old_total = Decimal::from_str(old_state[1])?;
                                                    cpuinfos.push(format!(
                                                        "{}{:.0}%",
                                                        cpu_freq_icon(cpu_no).unwrap_or("".to_string()),
                                                        *DECIMAL_100 * (used - old_used) / (total - old_total)
                                                    ));
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
