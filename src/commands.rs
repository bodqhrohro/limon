extern crate procfs;
extern crate lazy_static;
extern crate linereader;
extern crate regex;
extern crate rust_decimal;
extern crate once_cell;
extern crate sensors;
extern crate hdd;

use std::env;
use std::fs;
use std::path;
use std::io;
use std::io::Write;
use std::str::FromStr;
use std::collections::BTreeMap;
use std::process;

use lazy_static::lazy_static;
use linereader::LineReader;
use regex::Regex;
use rust_decimal::Decimal;
use once_cell::sync::OnceCell;
use sensors::Sensors;
use hdd::ata::ATADevice;
use hdd::scsi::SCSIDevice;
use hdd::ata::misc::Misc;
use hdd::ata::data::attr::raw::Raw as HDDRaw;

pub struct Command
{
    pub icon: char,
    pub call: fn(&[&str]) -> Option<String>,
}

const FILE_PREFIX: &str = ".limon-";
macro_rules! TEMPERATURE_FORMAT { () => { "{:+.1}°C" }; }

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

fn trim_trailing_newline(s: &mut String) -> () {
    if s.ends_with('\n') {
        s.pop();
    }
}

fn persist_state(name: &str, save: &str) -> io::Result<String> {
    let mut file_path_buf = TEMP_DIR.clone();
    file_path_buf.push(FILE_PREFIX.to_owned() + name);
    let file_path = file_path_buf.as_path();

    let mut prev_state = String::new();

    // nothing bad if the file doesn't exist, just return an empty state
    if let Ok(contents) = fs::read_to_string(file_path) {
        prev_state = contents;
    }

    let new_file = fs::File::create(file_path)?;
    write!(&new_file, "{}", save)?;

    Ok(prev_state)
}

fn read_u32_from_file(filename: &str) -> io::Result<u32> {
    let mut contents = fs::read_to_string(filename)?;

    trim_trailing_newline(&mut contents);

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

#[derive(Clone)]
struct Traffic {
    rx: u64,
    tx: u64,
    rx_string: String,
    tx_string: String,
    iface: String,
}
type MaybeTraffic = Result<Traffic, String>;
static RX_TX: OnceCell<MaybeTraffic> = OnceCell::new();

fn fetch_traffic(iface: &str) -> MaybeTraffic {
    let path_base = "/sys/class/net/".to_string() + iface + "/statistics/";

    // read rx
    match fs::read_to_string(path_base.to_owned() + "rx_bytes") {
        Ok(mut rx_string) => {
            trim_trailing_newline(&mut rx_string);
            // read tx
            if let Ok(mut tx_string) = fs::read_to_string(path_base + "tx_bytes") {
                trim_trailing_newline(&mut tx_string);

                if let Ok(result) = || -> Result<Traffic, std::num::ParseIntError> {
                    let rx = u64::from_str(&rx_string)?;
                    let tx = u64::from_str(&tx_string)?;

                    Ok(Traffic {
                        rx: rx,
                        tx: tx,
                        rx_string: rx_string,
                        tx_string: tx_string,
                        iface: iface.to_string(),
                    })
                }() {
                    return Ok(result);
                }
            }
        },
        Err(_) => {
            // reference to http://web.archive.org/web/20130430040505/http://promodj.com/cybersatan/tracks/4073655/ZB_CyberSatan_TDPLM_Akti_2_3_Otkrovenie_i_Problemi_s_Setyu :)
            return Err("Дисконнект, б**".to_string())
        }
    }

    Err("".to_string())
}

fn update_traffic(iface: &str) -> MaybeTraffic {
    let traffic = fetch_traffic(iface);

    // Err means that the cell isn't empty, ignore it
    match RX_TX.set(traffic.clone()) {
        Ok(_) => {},
        Err(_) => {}
    }

    traffic
}

fn fetch_traffic_cached(iface: &str) -> MaybeTraffic {
    match RX_TX.get() {
        Some(rx_tx) => {
            match (*rx_tx).clone() {
                Ok(rx_tx) => {
                    // return the cached result
                    if rx_tx.iface == iface {
                        Ok(rx_tx)
                    // fetch for another interface, don't touch the cache
                    } else {
                        fetch_traffic(iface)
                    }
                },
                // fetched with an error first time, try again
                Err(_) => {
                    update_traffic(iface)
                }
            }
        },
        // wasn't fetched yet, fetch and cache
        None => {
            update_traffic(iface)
        }
    }
}

lazy_static! {
    static ref SENSORS: Sensors = Sensors::new();
}
fn get_chip_temperature(chip_name: &str, temperature_name: &str) -> Option<String> {
    if let Ok(mut chip_iter) = (*SENSORS).detected_chips(chip_name) {
        // just pick the first chip there
        let chip = chip_iter.next();
        if let Some(chip) = chip {
            if let Some(feat) = chip.into_iter().find(|feat| feat.name() == temperature_name) {
                if let Some(subfeat) = feat.get_subfeature(sensors::SubfeatureType::SENSORS_SUBFEATURE_TEMP_INPUT) {
                    if let Ok(value) = subfeat.get_value() {
                        return Some(format!(TEMPERATURE_FORMAT!(), value));
                    }
                }
            }
        }
    }

    None
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
        if let Ok(stat_file) = fs::File::open("/proc/stat") {
            let mut linereader = LineReader::new(stat_file);

            let mut cpuinfos: Vec<String> = vec![];

            if let Ok(_) = linereader.for_each(|line| {
                if let Ok(str_line) = std::str::from_utf8(line) {
                    if let Some(caps) = CPU_LINE_REGEXP.captures(str_line) {
                        let a: Vec<&str> = str_line.split(" ").collect();
                        if a.len() >= 9 {
                            if let Ok(_) = || -> Result<(), rust_decimal::Error> {
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
                                if let Ok(old_state) = persist_state(&("old".to_owned() + a[0]), &new_state) {
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
                                };

                                Ok(())
                            }() {};
                        }
                    }
                }

                Ok(true)
            }) {
                return Some(cpuinfos.join(" "));
            }
        }

        None
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

pub const ZRAM:Command = Command {
    icon: '',
    call: |_| {
        match &*MEMINFO {
            Ok(meminfo) => {
                let swap_free = meminfo.swap_free;
                let swap_total = meminfo.swap_total;

                let mut total_comp: u64 = 0;

                for i in 0.. {
                    match fs::read_to_string("/sys/devices/virtual/block/zram".to_string() + &i.to_string() + "/mm_stat") {
                        Ok(contents) => {
                            let a: Vec<&str> = contents.split(" ").collect();
                            if a.len() >= 2 {
                                if let Ok(comp) = u64::from_str(a[1]) {
                                    total_comp += comp;
                                }
                            }
                        },
                        // assume that the numeration is contiguous, so if the file
                        // on this interation can't be opened then all they were passed
                        Err(_) => { break; }
                    }
                }

                Some(format_two_amounts(total_comp, swap_total - swap_free, ":"))
            },
            Err(_) => None,
        }
    },
};

const RADEON_VRAM_BLOCK_SIZE: u64 = 4096;
pub const RADEON_VRAM:Command = Command {
    icon: '',
    call: |_| {
        // there's no too much contents (around 32 kB for me), so it's easier
        // to read it all rather than messing with LineReader and copying lines
        // crawled by it
        let output = process::Command::new("sudo")
            .args(&["cat", "/sys/kernel/debug/dri/0/radeon_vram_mm"])
            .output();

        if let Ok(output) = output {
            if let Ok(stdout) = std::str::from_utf8(&output.stdout) {
                let last_line = stdout.lines().last();

                if let Some(last_line) = last_line {
                    let a: Vec<&str> = last_line.split(" ").collect();
                    if a.len() >= 4 {
                        if let Ok(result) = || -> Result<String, std::num::ParseIntError> {
                            let used = u64::from_str(a[3])?;

                            let mut total = a[1].to_string();
                            // strip the comma
                            total.pop();
                            let total = u64::from_str(&total)?;

                            Ok(format_two_amounts(used * RADEON_VRAM_BLOCK_SIZE, total * RADEON_VRAM_BLOCK_SIZE, "/"))
                        }() {
                            return Some(result);
                        }
                    }
                }
            }
        }

        None
    },
};

pub const TRAFFIC:Command = Command {
    icon: '',
    call: |args| {
        if args.len() < 1 {
            return None;
        }

        let traffic = fetch_traffic_cached(args[0]);

        match traffic {
            Ok(traffic) => Some(format_two_amounts(traffic.rx, traffic.tx, ":")),
            Err(msg) => Some(msg)
        }
    },
};

pub const NETWORK_SPEED:Command = Command {
    icon: '',
    call: |args| {
        if args.len() < 1 {
            return None;
        }

        let traffic = fetch_traffic_cached(args[0]);

        match traffic {
            Ok(traffic) => {
                // save anyway, display only if there was an old state
                let new_state = format!("{} {}", traffic.rx_string, traffic.tx_string);
                if let Ok(old_state) = persist_state("network-speed-stat", &new_state) {
                    let old_state: Vec<&str> = old_state.split(" ").collect();
                    if old_state.len() == 2 {
                        if let Ok(result) = || -> Result<String, std::num::ParseIntError> {
                            let new_rx = traffic.rx;
                            let new_tx = traffic.tx;
                            let old_rx = u64::from_str(old_state[0])?;
                            let old_tx = u64::from_str(old_state[1])?;

                            // TODO: fix a possible panic here
                            Ok(format_two_amounts(new_rx - old_rx, new_tx - old_tx, ":"))
                        }() {
                            return Some(result);
                        }
                    }
                }

                None
            },
            Err(msg) => Some(msg)
        }
    },
};

pub const RADEON_TEMPERATURE:Command = Command {
    icon: '',
    call: |_| {
        get_chip_temperature("radeon-pci-0100", "temp1")
    },
};

pub const AMD_K10_TEMPERATURE:Command = Command {
    icon: '',
    call: |_| {
        get_chip_temperature("k10temp-pci-00c3", "temp1")
    },
};

const TEMPERATURE_CELSIUS: u8 = 194;
pub const ATA_HDDTEMP:Command = Command {
    icon: '',
    call: |args| {
        if args.len() < 1 {
            return None;
        }

        if let Ok(device) = hdd::device::linux::Device::open(args[0]) {
            let ata_device = ATADevice::new(SCSIDevice::new(device));
            if let Ok(attrs) = ata_device.get_smart_attributes(&None) {
                if let Some(attr) = attrs.iter().find(|attr| attr.id == TEMPERATURE_CELSIUS) {
                    return match attr.raw {
                        HDDRaw::CelsiusMinMax{current, ..} =>
                            return Some(format!(TEMPERATURE_FORMAT!(), current as f64)),
                        // the value seems to be 0x00000max0min0cur
                        HDDRaw::Raw64(raw) =>
                            return Some(format!(TEMPERATURE_FORMAT!(), (raw & 0xff) as f64)),
                        _ => None
                    }
                }
            }
        }

        None
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
