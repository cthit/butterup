#[macro_use]
extern crate log;

mod actions;
mod local;
mod planner;
mod remote;
mod util;

use actions::{list, show_plan, sync};
use chrono::{DateTime, FixedOffset};
use clap::{crate_version, Parser};
use log::LevelFilter;
use remote::Remote;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub type TimeStamp = DateTime<FixedOffset>;
pub type FileList = BTreeMap<TimeStamp, String>;

/// Backup btrfs snapshots over SSH
#[derive(Parser)]
#[clap(version = crate_version!())]
pub struct Opt {
    /// Log more stuff
    #[clap(long, short, parse(from_occurrences))]
    verbose: u8,

    /// Do not output anything but errors.
    #[clap(long, short)]
    quiet: bool,

    /// The path of the backup directory on the local filesystem
    #[clap(short = 'l', long)]
    path: PathBuf,

    /// The ssh remote for the backup (user@host:port:path)
    #[clap(short, long)]
    remote: Remote,

    /// The SSH privkey file for the remote
    #[clap(long)]
    privkey: PathBuf,

    /// The password for the SSH privkey file
    #[clap(long)]
    privkey_pass: Option<String>,

    #[clap(subcommand)]
    action: Action,
}

#[derive(Parser)]
pub enum Action {
    /// Perform a backup
    Backup {
        /// Backup all files, not just the most recent ones
        #[clap(long)]
        all: bool,
    },

    /// Generate and show a backup plan
    ShowPlan {
        /// Backup all files, not just the most recent ones
        #[clap(long)]
        all: bool,
    },

    /// List all backups, and where they reside
    List,
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    let log_level = match opt.verbose {
        0 if opt.quiet => LevelFilter::Error,
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        2.. => LevelFilter::Trace,
    };

    pretty_env_logger::formatted_builder()
        .filter(None, log_level)
        .init();

    match opt.action {
        Action::Backup { all } => sync::run(&opt, all)?,
        Action::ShowPlan { all } => show_plan::run(&opt, all)?,
        Action::List => list::run(&opt)?,
    }

    Ok(())
}
