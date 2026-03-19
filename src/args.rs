use clap::{Arg, Command};
use std::sync::LazyLock;

pub struct Args {
    pub port: u16,
    pub fps: u16,
    pub max_clients: u32,
    pub debug: bool,
}

pub fn get_args() -> &'static Args {
    static ARGS: LazyLock<Args> = LazyLock::new(|| parse_args());
    &ARGS
}

fn parse_args() -> Args {
    let matches = Command::new("webds")
        .about("webds - web desktop share")
        .arg(
            Arg::new("port")
                .long("port")
                .default_value("3000")
                .value_parser(clap::value_parser!(u16)),
        )
        .arg(
            Arg::new("fps")
                .long("fps")
                .default_value("30")
                .value_parser(clap::value_parser!(u16)),
        )
        .arg(
            Arg::new("max clients")
                .long("max")
                .default_value("5")
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .short('d')
                .action(clap::ArgAction::SetTrue),
        )
        .after_help("example: webds --port 4000 --fps 30 --max 50")
        .get_matches();

    Args {
        port: *matches.get_one("port").unwrap(),
        fps: *matches.get_one("fps").unwrap(),
        max_clients: *matches.get_one("max clients").unwrap(),
        debug: matches.get_flag("debug"),
    }
}
