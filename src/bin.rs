extern crate limonlib;
extern crate arguments;

use std::env;

use limonlib::{LimonItem, exec_command, commands};

struct CommandAndArgs<'a> {
    command: &'a commands::Command,
    args: &'a [&'a str],
}

pub fn main() {
    let args = env::args();
    let args = arguments::parse(args).unwrap();

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
        CommandAndArgs{command: &commands::Command::Static(commands::ATA_GSENSE_ERROR_RATE), args: &["/dev/sda"]},
        CommandAndArgs{command: &commands::Command::Static(commands::NETWORK_SPEED), args: &wireless_interface},
        CommandAndArgs{command: &commands::Command::Static(commands::WIRELESS_SIGNAL), args: &wireless_interface},
        CommandAndArgs{command: &commands::Command::Static(commands::DISK_IO_SPEED), args: &["sda"]},
        CommandAndArgs{command: &commands::Command::Static(commands::FS_FREE), args: &["/"]},
        CommandAndArgs{command: &commands::Command::Static(commands::UPS_VOLTAGE), args: &["nutdev"]},
        CommandAndArgs{command: &commands::Command::Dynamic(commands::BATTERY), args: &[]},
    ];

    let results: Vec<LimonItem> = cmds.iter().map(|cmd| exec_command(cmd.command, cmd.args)).collect();
    let bar = match results.last() {
        Some(item) => item.bar,
        _ => None,
    };

    if args.get::<bool>("pango") == Some(true) {
        limonlib::output_pango(results, 12, 11, bar);
    } else {
        limonlib::output_plain(results);
    }
}
