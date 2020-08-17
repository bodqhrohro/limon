extern crate limonlib;

use limonlib::exec_command;

pub fn main() {
    let loadavg = exec_command(&limonlib::commands::LOADAVG, &[]);
    let cpu = exec_command(&limonlib::commands::CPU, &[]);
    let mem = exec_command(&limonlib::commands::MEM, &[]);
    let zram = exec_command(&limonlib::commands::ZRAM, &[]);
    let radeon_vram = exec_command(&limonlib::commands::RADEON_VRAM, &[]);
    let traffic = exec_command(&limonlib::commands::TRAFFIC, &["wlan0"]);
    let network_speed = exec_command(&limonlib::commands::NETWORK_SPEED, &["wlan0"]);
    let radeon_temperature = exec_command(&limonlib::commands::RADEON_TEMPERATURE, &[]);
    let amd_k10_temperature = exec_command(&limonlib::commands::AMD_K10_TEMPERATURE, &[]);
    let ata_hddtemp = exec_command(&limonlib::commands::ATA_HDDTEMP, &["/dev/sda"]);
    limonlib::output_pango(vec!(loadavg, cpu, mem, zram, radeon_vram, traffic, network_speed, radeon_temperature, amd_k10_temperature, ata_hddtemp), 12);
}
