extern crate limonlib;

use limonlib::{exec_command, commands};

struct CommandAndArgs<'a> {
    command: &'a commands::Command,
    args: &'a [&'a str],
}

pub fn main() {
    let wireless_interface = ["wlan0"];

    let cmds = vec![
        CommandAndArgs{command: &commands::Command::Static(commands::LOADAVG), args: &[]},
        CommandAndArgs{command: &commands::Command::Static(commands::CPU), args: &[]},
        CommandAndArgs{command: &commands::Command::Static(commands::MEM), args: &[]},
        CommandAndArgs{command: &commands::Command::Static(commands::ZRAM), args: &[]},
        CommandAndArgs{command: &commands::Command::Static(commands::RADEON_VRAM), args: &[]},
        CommandAndArgs{command: &commands::Command::Static(commands::TRAFFIC), args: &wireless_interface},
        CommandAndArgs{command: &commands::Command::Static(commands::RADEON_TEMPERATURE), args: &[]},
        CommandAndArgs{command: &commands::Command::Static(commands::AMD_K10_TEMPERATURE), args: &[]},
        CommandAndArgs{command: &commands::Command::Static(commands::ATA_HDDTEMP), args: &["/dev/sda"]},
        CommandAndArgs{command: &commands::Command::Static(commands::NETWORK_SPEED), args: &wireless_interface},
        CommandAndArgs{command: &commands::Command::Static(commands::WIRELESS_SIGNAL), args: &wireless_interface},
        CommandAndArgs{command: &commands::Command::Static(commands::DISK_IO_SPEED), args: &["sda"]},
        CommandAndArgs{command: &commands::Command::Static(commands::FS_FREE), args: &["/"]},
    ];

    limonlib::output_pango(cmds.iter().map(|cmd| exec_command(cmd.command, cmd.args)).collect(), 12);
}
