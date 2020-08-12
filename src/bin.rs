extern crate limonlib;

use limonlib::exec_command;

pub fn main() {
    let loadavg = exec_command(&limonlib::commands::LOADAVG, &[]);
    let cpu = exec_command(&limonlib::commands::CPU, &[]);
    let mem = exec_command(&limonlib::commands::MEM, &[]);
    limonlib::output_pango(vec!(loadavg, cpu, mem), 12);
}
