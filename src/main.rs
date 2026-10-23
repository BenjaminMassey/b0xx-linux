mod app;
mod config;
mod configure;
mod gamepad;
mod input;
mod keys;
mod logic;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use gamepad::DolphinFlavor;
use std::path::PathBuf;

const DOLPHIN_PROFILE: &str = include_str!("../dolphin/b0xx-linux.ini");
const PROFILE_FILE_NAME: &str = "b0xx-linux.ini";

/// Emulates a B0XX controller with a keyboard, for Dolphin on Linux.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Hotkeys file (same format as the AutoHotkey version's hotkeys.ini).
    /// Defaults to ~/.config/b0xx-linux/hotkeys.ini, created if missing.
    #[arg(long, short, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    run: RunArgs,
}

#[derive(clap::Args)]
struct RunArgs {
    /// Only read this keyboard (e.g. /dev/input/by-id/...-event-kbd). Repeatable.
    /// By default every keyboard is read, including ones plugged in later.
    #[arg(long, short)]
    device: Vec<PathBuf>,

    /// Which Dolphin will read the controller. This changes how stick positions are
    /// encoded so that Melee receives the exact B0XX coordinates.
    #[arg(long, value_enum, default_value = "ishiiruka")]
    dolphin: DolphinFlavor,

    /// Print every button press and release.
    #[arg(long, short)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Run the controller emulator (the default).
    Run(RunArgs),
    /// Interactively bind keys to buttons.
    Configure {
        /// Only listen to this keyboard. Repeatable.
        #[arg(long, short)]
        device: Vec<PathBuf>,
    },
    /// List input devices.
    ListDevices,
    /// Install the Dolphin controller profile into Slippi / Dolphin's config.
    InstallProfile {
        /// Profiles directory to install into, instead of the detected ones.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Print the Dolphin controller profile.
    PrintProfile,
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = cli.config.unwrap_or_else(config::default_path);
    match cli.command {
        None => run(cli.run, config_path),
        Some(Command::Run(args)) => run(args, config_path),
        Some(Command::Configure { device }) => configure::run(&config_path, device),
        Some(Command::ListDevices) => {
            input::list_devices();
            Ok(())
        }
        Some(Command::InstallProfile { dir }) => install_profile(dir),
        Some(Command::PrintProfile) => {
            print!("{DOLPHIN_PROFILE}");
            Ok(())
        }
    }
}

fn run(args: RunArgs, config_path: PathBuf) -> Result<()> {
    app::run(app::RunOptions { config_path, devices: args.device, flavor: args.dolphin, verbose: args.verbose })
}

fn install_profile(dir: Option<PathBuf>) -> Result<()> {
    let dirs = match dir {
        Some(dir) => vec![dir],
        None => {
            let home = PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?);
            // (Dolphin user directory, profiles directory within it)
            let candidates = [
                (".config/SlippiOnline", "Config/Profiles/GCPad"),
                (".config/dolphin-emu", "Profiles/GCPad"),
                (".var/app/org.DolphinEmu.dolphin-emu/config/dolphin-emu", "Profiles/GCPad"),
            ];
            let found: Vec<_> = candidates
                .iter()
                .filter(|(user, _)| home.join(user).is_dir())
                .map(|(user, profiles)| home.join(user).join(profiles))
                .collect();
            anyhow::ensure!(
                !found.is_empty(),
                "no Slippi or Dolphin config directory found; run Dolphin once, or pass --dir"
            );
            found
        }
    };
    for dir in dirs {
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join(PROFILE_FILE_NAME);
        std::fs::write(&path, DOLPHIN_PROFILE).with_context(|| format!("writing {}", path.display()))?;
        println!("Installed {}", path.display());
    }
    println!("In Dolphin: Controllers > Port 1: Standard Controller > Configure > Profile: b0xx-linux > Load.");
    Ok(())
}
