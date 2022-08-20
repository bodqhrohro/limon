extern crate procfs;
extern crate lazy_static;
extern crate linereader;
extern crate regex;
extern crate rust_decimal;
extern crate once_cell;
extern crate sensors;
extern crate hdd;
extern crate itertools;
extern crate libc;
extern crate battery;

use std::env;
use std::fs;
use std::path;
use std::io;
use std::io::Write;
use std::str::FromStr;
use std::collections::BTreeMap;
use std::process;
use std::mem;
use std::ffi::CString;

use super::utils::trim_trailing_newline;

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
use itertools::free::join;

pub struct StaticIconCommand
{
    pub icon: char,
    pub call: fn(&[&str]) -> Option<String>,
}

pub struct DynamicIconCommandOutput
{
    pub icon: char,
    pub text: String,
    pub bar: Option<u8>,
}

pub struct DynamicIconCommand
{
    pub call: fn(&[&str]) -> Option<DynamicIconCommandOutput>,
}

pub enum Command {
    Static(StaticIconCommand),
    Dynamic(DynamicIconCommand),
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
    static ref BYTE_SUFFIX_MAP: BTreeMap<usize, &'static str> = {
        let mut map = BTreeMap::new();
        map.insert(10 * (1 << 10), "K");
        map.insert(10 * (1 << 20), "M");
        map.insert(10 * (1 << 30), "G");
        map.insert(10 * (1 << 40), "T");
        map.insert(10 * (1 << 50), "P");
        map
    };
}
fn format_two_amounts(a1: usize, a2: usize, separator: &str) -> String {
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
    rx: usize,
    tx: usize,
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
                    let rx = usize::from_str(&rx_string)?;
                    let tx = usize::from_str(&tx_string)?;

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

lazy_static! {
    static ref DBMS_LEVELS: BTreeMap<i16, char> = {
        let mut map = BTreeMap::new();
        map.insert(-90, '▁');
        map.insert(-80, '▂');
        map.insert(-70, '▃');
        map.insert(-67, '▄');
        map.insert(-60, '▅');
        map
    };
}
fn show_dbms(dbms: i16) -> String {
    join(DBMS_LEVELS.keys().map(|floor| {
        if dbms >= *floor {
            if let Some(bar) = DBMS_LEVELS.get(floor) {
                return bar;
            }
        }

        &' '
    }), &"")
}

const BATTERY_LEVELS: [(u8, char); 4] = [
    (80, ''),
    (60, ''),
    (40, ''),
    (20, ''),
];
fn show_battery_icon(level: u8) -> char {
    for (floor, icon) in BATTERY_LEVELS.iter() {
        if level >= *floor {
            return *icon;
        }
    }

    ''
}



pub const LOADAVG:StaticIconCommand = StaticIconCommand {
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
pub const CPU:StaticIconCommand = StaticIconCommand {
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
    static ref MEMINFO: procfs::ProcResult<procfs::Meminfo> = procfs::Meminfo::new();
}
pub const MEM:StaticIconCommand = StaticIconCommand {
    icon: '',
    call: |_| {
        match &*MEMINFO {
            Ok(meminfo) => {
                let mem_available = meminfo.mem_available.unwrap_or(0) as usize;
                let mem_total = meminfo.mem_total as usize;

                Some(format_two_amounts(mem_total - mem_available, mem_total, "/"))
            },
            Err(_) => None,
        }
    },
};

pub const ZRAM:StaticIconCommand = StaticIconCommand {
    icon: '',
    call: |_| {
        match &*MEMINFO {
            Ok(meminfo) => {
                let swap_free = meminfo.swap_free;
                let swap_total = meminfo.swap_total;

                let mut total_comp: usize = 0;

                for i in 0.. {
                    match fs::read_to_string("/sys/devices/virtual/block/zram".to_string() + &i.to_string() + "/mm_stat") {
                        Ok(contents) => {
                            let a: Vec<&str> = contents.split(" ").collect();
                            if a.len() >= 2 {
                                if let Ok(comp) = usize::from_str(a[1]) {
                                    total_comp += comp;
                                }
                            }
                        },
                        // assume that the numeration is contiguous, so if the file
                        // on this interation can't be opened then all they were passed
                        Err(_) => { break; }
                    }
                }

                Some(format_two_amounts(total_comp, (swap_total - swap_free) as usize, ":"))
            },
            Err(_) => None,
        }
    },
};

const RADEON_VRAM_BLOCK_SIZE: usize = 4096;
pub const RADEON_VRAM:StaticIconCommand = StaticIconCommand {
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
                            let used = usize::from_str(a[3])?;

                            let mut total = a[1].to_string();
                            // strip the comma
                            total.pop();
                            let total = usize::from_str(&total)?;

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

pub const TRAFFIC:StaticIconCommand = StaticIconCommand {
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

pub const NETWORK_SPEED:StaticIconCommand = StaticIconCommand {
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
                            let old_rx = usize::from_str(old_state[0])?;
                            let old_tx = usize::from_str(old_state[1])?;

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

pub const RADEON_TEMPERATURE:StaticIconCommand = StaticIconCommand {
    icon: '',
    call: |_| {
        get_chip_temperature("radeon-pci-0100", "temp1")
    },
};

pub const AMD_K10_TEMPERATURE:StaticIconCommand = StaticIconCommand {
    icon: '',
    call: |_| {
        get_chip_temperature("k10temp-pci-00c3", "temp1")
    },
};

const TEMPERATURE_CELSIUS: u8 = 194;
pub const ATA_HDDTEMP:StaticIconCommand = StaticIconCommand {
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

pub const WIRELESS_SIGNAL:StaticIconCommand = StaticIconCommand {
    icon: '',
    call: |args| {
        if args.len() < 1 {
            return None;
        }

        let interface = args[0];

        if let Ok(stat_file) = fs::File::open("/proc/net/wireless") {
            let mut linereader = LineReader::new(stat_file);

            while let Some(Ok(line)) = linereader.next_line() {
                if let Ok(str_line) = std::str::from_utf8(line) {
                    let mut token_iter = str_line.split_whitespace();
                    if let Some(interface_column) = token_iter.next() {
                        if interface_column.starts_with(interface) {
                            if let Some(level) = token_iter.nth(2) {
                                let mut level = level.to_string();

                                if level.ends_with('.') {
                                    level.pop();
                                }

                                if let Ok(int_level) = level.parse::<i16>() {
                                    return Some(format!("{} {}", show_dbms(int_level), level));
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    },
};

const LINUX_BLOCK_SIZE: usize = 512;
pub const DISK_IO_SPEED:StaticIconCommand = StaticIconCommand {
    icon: '',
    call: |args| {
        if args.len() < 1 {
            return None;
        }

        let disk_name = args[0];

        if let Ok(diskstats) = procfs::diskstats() {
            if let Some(diskstat) = diskstats.iter().find(|diskstat| diskstat.name == args[0]) {
                let read_bytes = diskstat.sectors_read * LINUX_BLOCK_SIZE;
                let written_bytes = diskstat.sectors_written * LINUX_BLOCK_SIZE;

                let new_state = format!("{} {}", read_bytes, written_bytes);
                // save anyway, display only if there was an old state
                if let Ok(old_state) = persist_state(&("old".to_owned() + disk_name), &new_state) {
                    let old_state: Vec<&str> = old_state.split(" ").collect();
                    if old_state.len() == 2 {
                        if let Ok(result) = || -> Result<String, std::num::ParseIntError> {
                            let old_read_bytes = usize::from_str(old_state[0])?;
                            let old_written_bytes = usize::from_str(old_state[1])?;

                            Ok(format_two_amounts(read_bytes - old_read_bytes, written_bytes - old_written_bytes, ":"))
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

pub const FS_FREE:StaticIconCommand = StaticIconCommand {
    icon: '',
    call: |args| {
        if args.len() < 1 {
            return None;
        }

        let fs_root = args[0];

        if let Ok(c_fs_root) = CString::new(fs_root) {
            unsafe {
                let mut statvfs = mem::zeroed();
                if libc::statvfs(c_fs_root.as_ptr(), &mut statvfs) >= 0 {
                    let blocksize = if statvfs.f_frsize != 0 {
                        statvfs.f_frsize as usize
                    } else {
                        statvfs.f_bsize as usize
                    };
                    let free = blocksize * (statvfs.f_bavail as usize);
                    let total = blocksize * (statvfs.f_blocks as usize);

                    return Some(format_two_amounts(free, total, "/"));
                }
            }
        }

        None
    },
};

pub const BATTERY:DynamicIconCommand = DynamicIconCommand {
    call: |_| {
        if let Ok(manager) = battery::Manager::new() {
            if let Ok(mut batteries) = manager.batteries() {
                if let Some(Ok(battery)) = batteries.next() {
                    let state = battery.state_of_charge();
                    let int_state = (state.value * 100.0) as u8;

                    return Some(DynamicIconCommandOutput {
                        icon: show_battery_icon(int_state),
                        text: format!("{} %", int_state),
                        bar: Some(int_state),
                    });
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

    #[test]
    fn dbms_low() {
        let signal = show_dbms(-100);
        assert_eq!(signal, "     ");
    }

    #[test]
    fn dbms_91() {
        let signal = show_dbms(-91);
        assert_eq!(signal, "     ");
    }

    #[test]
    fn dbms_90() {
        let signal = show_dbms(-90);
        assert_eq!(signal, "▁    ");
    }

    #[test]
    fn dbms_89() {
        let signal = show_dbms(-89);
        assert_eq!(signal, "▁    ");
    }

    #[test]
    fn dbms_81() {
        let signal = show_dbms(-81);
        assert_eq!(signal, "▁    ");
    }

    #[test]
    fn dbms_80() {
        let signal = show_dbms(-80);
        assert_eq!(signal, "▁▂   ");
    }

    #[test]
    fn dbms_79() {
        let signal = show_dbms(-79);
        assert_eq!(signal, "▁▂   ");
    }

    #[test]
    fn dbms_71() {
        let signal = show_dbms(-71);
        assert_eq!(signal, "▁▂   ");
    }

    #[test]
    fn dbms_70() {
        let signal = show_dbms(-70);
        assert_eq!(signal, "▁▂▃  ");
    }

    #[test]
    fn dbms_69() {
        let signal = show_dbms(-69);
        assert_eq!(signal, "▁▂▃  ");
    }

    #[test]
    fn dbms_68() {
        let signal = show_dbms(-68);
        assert_eq!(signal, "▁▂▃  ");
    }

    #[test]
    fn dbms_67() {
        let signal = show_dbms(-67);
        assert_eq!(signal, "▁▂▃▄ ");
    }

    #[test]
    fn dbms_66() {
        let signal = show_dbms(-66);
        assert_eq!(signal, "▁▂▃▄ ");
    }

    #[test]
    fn dbms_61() {
        let signal = show_dbms(-61);
        assert_eq!(signal, "▁▂▃▄ ");
    }

    #[test]
    fn dbms_60() {
        let signal = show_dbms(-60);
        assert_eq!(signal, "▁▂▃▄▅");
    }

    #[test]
    fn dbms_59() {
        let signal = show_dbms(-59);
        assert_eq!(signal, "▁▂▃▄▅");
    }

    #[test]
    fn dbms_large() {
        let signal = show_dbms(-20);
        assert_eq!(signal, "▁▂▃▄▅");
    }

    #[test]
    fn dbms_zero() {
        let signal = show_dbms(0);
        assert_eq!(signal, "▁▂▃▄▅");
    }

    #[test]
    fn dbms_positive() {
        let signal = show_dbms(40);
        assert_eq!(signal, "▁▂▃▄▅");
    }

    #[test]
    fn trim_newline() {
        let mut s = "a\nb\n".to_string();
        trim_trailing_newline(&mut s);
        assert_eq!(s, "a\nb".to_string());
    }

    #[test]
    fn trim_no_newline() {
        let mut s = "a\nb\nc".to_string();
        trim_trailing_newline(&mut s);
        assert_eq!(s, "a\nb\nc".to_string());
    }

    #[test]
    fn battery_overfull() {
        let level = show_battery_icon(1000.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_255() {
        let level = show_battery_icon(255.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_101() {
        let level = show_battery_icon(101.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_100() {
        let level = show_battery_icon(100.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_99() {
        let level = show_battery_icon(99.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_81() {
        let level = show_battery_icon(81.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_80() {
        let level = show_battery_icon(80.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_79() {
        let level = show_battery_icon(79.9 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_61() {
        let level = show_battery_icon(61.1 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_60() {
        let level = show_battery_icon(60.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_59() {
        let level = show_battery_icon(59.9 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_41() {
        let level = show_battery_icon(41.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_40() {
        let level = show_battery_icon(40.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_39() {
        let level = show_battery_icon(39.9 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_21() {
        let level = show_battery_icon(21.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_20() {
        let level = show_battery_icon(20.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_19() {
        let level = show_battery_icon(19.9 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_1() {
        let level = show_battery_icon(1.9 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_0() {
        let level = show_battery_icon(0.0 as u8);
        assert_eq!(level, '');
    }

    #[test]
    fn battery_negative() {
        let level = show_battery_icon(-23.0 as u8);
        assert_eq!(level, '');
    }
}
