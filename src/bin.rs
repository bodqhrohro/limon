extern crate limonlib;

use limonlib::exec_command;

pub fn main() {
    let loadavg = exec_command(&limonlib::commands::LOADAVG, &[]);
    let cpu = exec_command(&limonlib::commands::CPU, &[]);
    let mem = exec_command(&limonlib::commands::MEM, &[]);
    let zram = exec_command(&limonlib::commands::ZRAM, &[]);
    let radeon_vram = exec_command(&limonlib::commands::RADEON_VRAM, &[]);
    limonlib::output_pango(vec!(loadavg, cpu, mem, zram, radeon_vram), 12);
}
