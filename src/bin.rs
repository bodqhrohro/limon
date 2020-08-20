extern crate limonlib;

use limonlib::exec_command;

pub fn main() {
    let wireless_interface = "wlan0";

    let loadavg = exec_command(&limonlib::commands::LOADAVG, &[]);
    let cpu = exec_command(&limonlib::commands::CPU, &[]);
    let mem = exec_command(&limonlib::commands::MEM, &[]);
    let zram = exec_command(&limonlib::commands::ZRAM, &[]);
    let radeon_vram = exec_command(&limonlib::commands::RADEON_VRAM, &[]);
    let traffic = exec_command(&limonlib::commands::TRAFFIC, &[wireless_interface]);
    let network_speed = exec_command(&limonlib::commands::NETWORK_SPEED, &[wireless_interface]);
    let radeon_temperature = exec_command(&limonlib::commands::RADEON_TEMPERATURE, &[]);
    let amd_k10_temperature = exec_command(&limonlib::commands::AMD_K10_TEMPERATURE, &[]);
    let ata_hddtemp = exec_command(&limonlib::commands::ATA_HDDTEMP, &["/dev/sda"]);
    let wireless_signal = exec_command(&limonlib::commands::WIRELESS_SIGNAL, &[wireless_interface]);
    let disk_io_speed = exec_command(&limonlib::commands::DISK_IO_SPEED, &["sda"]);
    let fs_free = exec_command(&limonlib::commands::FS_FREE, &["/"]);

    limonlib::output_pango(vec!(loadavg, cpu, mem, zram, radeon_vram, traffic, radeon_temperature, amd_k10_temperature, ata_hddtemp, network_speed, wireless_signal, disk_io_speed, fs_free), 12);
}
