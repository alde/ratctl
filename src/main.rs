mod config;
mod device;
mod devices;
mod evdev;
mod corsair;
mod protocol;
mod protocols;
mod razer;
mod tui;
mod types;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use crate::devices::{DeviceDescriptor, ProtocolKind};
use crate::protocols::MouseProtocol;

#[derive(Parser)]
#[command(name = "ratctl", about = "Configure gaming mice on Linux")]
struct Cli {
    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Path to hidraw device (auto-detected if omitted)
    #[arg(short, long)]
    device: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Read device state and write to config file
    Dump {
        /// Only dump this profile (1-5). Default: all profiles.
        #[arg(long)]
        profile: Option<u8>,
    },
    /// Apply config file to device
    Apply {
        /// Only apply this profile (1-5). Default: all in config.
        #[arg(long)]
        profile: Option<u8>,
    },
    /// Print current device settings
    Status,
    /// Interactive TUI configurator
    Tui,
    /// Switch active profile on device
    Profile {
        /// Profile number (1-5)
        index: u8,
    },
    /// List supported devices
    Devices,
}

fn open_protocol(
    detected: &device::DetectedDevice,
) -> Result<Box<dyn MouseProtocol>> {
    match detected.descriptor.protocol {
        ProtocolKind::Asus => {
            let dev = device::HidrawDevice::open(&detected.path)?;
            Ok(Box::new(protocols::asus::AsusProtocol::new(dev)))
        }
        ProtocolKind::Razer => {
            let path = razer::find_razer_hidraw(detected.descriptor)?;
            let dev = razer::RazerDevice::open(&path)?;
            Ok(Box::new(protocols::razer::RazerProtocol::new(dev)))
        }
        ProtocolKind::Corsair => {
            let path = corsair::find_corsair_hidraw(detected.descriptor)?;
            let dev = corsair::CorsairDevice::open(&path)?;
            Ok(Box::new(protocols::corsair::CorsairProtocol::new(dev)))
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Command::Devices = cli.command {
        return cmd_devices();
    }

    let config_path = cli.config.unwrap_or_else(config::default_config_path);

    let detected = match cli.device {
        Some(ref p) => device::detect_from_path(p)?,
        None => device::find_device()?,
    };

    let desc = detected.descriptor;
    println!("Found: {} ({})", desc.name, detected.path.display());

    let mut proto = open_protocol(&detected)?;

    match cli.command {
        Command::Dump { profile } => cmd_dump(proto.as_mut(), desc, &config_path, profile),
        Command::Apply { profile } => cmd_apply(proto.as_mut(), desc, &config_path, profile),
        Command::Status => cmd_status(proto.as_mut(), desc),
        Command::Tui => cmd_tui(proto.as_mut(), desc, &config_path),
        Command::Profile { index } => cmd_profile(proto.as_mut(), desc, index),
        Command::Devices => unreachable!(),
    }
}

fn cmd_devices() -> Result<()> {
    println!("Supported devices:");
    for desc in devices::REGISTRY {
        let pids: Vec<String> = desc.product_ids.iter().map(|p| format!("{:#06x}", p)).collect();
        let status = match desc.protocol {
            ProtocolKind::Asus => "ready",
            ProtocolKind::Razer => "ready (DPI/LED/polling)",
            ProtocolKind::Corsair => "ready (DPI/LED/polling)",
        };
        println!(
            "  {} [{}]  VID {:#06x}, PIDs: {}",
            desc.name, status, desc.vendor_id, pids.join(", ")
        );
    }
    Ok(())
}

fn cmd_dump(
    proto: &mut dyn MouseProtocol,
    desc: &DeviceDescriptor,
    config_path: &Path,
    profile_num: Option<u8>,
) -> Result<()> {
    let profile_data = proto.get_profile_data()?;
    println!(
        "Firmware: {}  Active profile: {}",
        profile_data.firmware_version,
        profile_data.current_profile + 1
    );

    let mut cfg = if config_path.exists() {
        config::load(config_path).unwrap_or_else(|e| {
            eprintln!("Warning: couldn't load existing config ({}), starting fresh", e);
            config::Config {
                active_profile: None,
                profile: Default::default(),
            }
        })
    } else {
        config::Config {
            active_profile: None,
            profile: Default::default(),
        }
    };

    cfg.active_profile = Some(profile_data.current_profile + 1);

    match profile_num {
        Some(n) => {
            validate_profile_num(n, desc)?;
            proto.set_profile(n - 1)?;
            let dp = proto.read_current_profile(desc)?;
            cfg.profile
                .insert(n.to_string(), config::profile_to_config(&dp, desc));
            proto.set_profile(profile_data.current_profile)?;
            println!("Dumped profile {}", n);
        }
        None => {
            let (_, profiles) = proto.read_all_profiles(desc)?;
            for (i, dp) in profiles.iter().enumerate() {
                cfg.profile
                    .insert((i + 1).to_string(), config::profile_to_config(dp, desc));
            }
            println!("Dumped all {} profiles", profiles.len());
        }
    }

    config::save(&cfg, config_path)?;
    println!("Config written to {}", config_path.display());
    Ok(())
}

fn cmd_apply(
    proto: &mut dyn MouseProtocol,
    desc: &DeviceDescriptor,
    config_path: &Path,
    profile_num: Option<u8>,
) -> Result<()> {
    let cfg = config::load(config_path)
        .with_context(|| format!("Failed to load config from {}", config_path.display()))?;

    let profile_data = proto.get_profile_data()?;
    let original_profile = profile_data.current_profile;

    if cfg.profile.is_empty() {
        eprintln!("Warning: config file contains no profiles. Nothing to apply.");
        return Ok(());
    }

    let profiles_to_apply: Vec<(u8, &config::ProfileConfig)> = match profile_num {
        Some(n) => {
            validate_profile_num(n, desc)?;
            let pc = cfg
                .profile
                .get(&n.to_string())
                .ok_or_else(|| anyhow::anyhow!("Profile {} not found in config file", n))?;
            vec![(n - 1, pc)]
        }
        None => {
            let mut v: Vec<_> = cfg
                .profile
                .iter()
                .filter_map(|(k, v)| k.parse::<u8>().ok().map(|n| (n - 1, v)))
                .collect();
            v.sort_by_key(|(idx, _)| *idx);
            v
        }
    };

    for (idx, pc) in &profiles_to_apply {
        let dp = config::config_to_profile(pc, desc)
            .with_context(|| format!("Invalid config for profile {}", idx + 1))?;
        proto.set_profile(*idx)?;
        proto.apply_profile(desc, &dp)?;
        proto.save()?;
        println!("Applied profile {}", idx + 1);
    }

    let target = cfg
        .active_profile
        .map(|n| n - 1)
        .unwrap_or(original_profile);
    proto.set_profile(target)?;
    println!("Done. Active profile: {}", target + 1);
    Ok(())
}

fn cmd_status(proto: &mut dyn MouseProtocol, desc: &DeviceDescriptor) -> Result<()> {
    let profile_data = proto.get_profile_data()?;
    println!("Firmware:       {}", profile_data.firmware_version);
    println!("Active profile: {}", profile_data.current_profile + 1);
    println!();

    let profile = proto.read_current_profile(desc)?;

    println!("=== Settings ===");
    println!("Polling rate:   {} Hz", profile.settings.polling_rate);
    println!("Debounce:       {} ms", profile.settings.debounce_ms);
    println!(
        "Angle snapping: {}",
        if profile.settings.angle_snapping { "on" } else { "off" }
    );
    println!();

    println!("=== DPI Presets ===");
    for (i, dpi) in profile.settings.dpi_presets.iter().enumerate() {
        println!("  Preset {}: {} DPI", i + 1, dpi);
    }
    println!();

    println!("=== Buttons ===");
    for (i, binding) in profile.buttons.iter().enumerate() {
        let slot_name = desc
            .button_slots
            .get(i)
            .map(|c| c.name())
            .unwrap_or("unknown");
        let action = config::format_binding(binding);
        println!("  {:12} -> {}", slot_name, action);
    }
    println!();

    println!("=== LEDs ===");
    for (i, led) in profile.leds.iter().enumerate() {
        let zone_name = desc.led_names.get(i).copied().unwrap_or("unknown");
        println!(
            "  {:6} mode={:10} brightness={}  color=#{:02x}{:02x}{:02x}",
            zone_name,
            led.mode.name(),
            led.brightness,
            led.r,
            led.g,
            led.b
        );
    }

    Ok(())
}

fn cmd_tui(
    proto: &mut dyn MouseProtocol,
    desc: &'static DeviceDescriptor,
    config_path: &Path,
) -> Result<()> {
    let mut evdev_dev = match evdev::find_mouse_evdev(desc) {
        Ok((path, dev)) => {
            eprintln!("Found evdev device at {}", path.display());
            Some(dev)
        }
        Err(e) => {
            eprintln!("Warning: evdev not available ({}). Button detection disabled.", e);
            None
        }
    };

    tui::run(proto, desc, evdev_dev.as_mut(), config_path)
}

fn cmd_profile(proto: &mut dyn MouseProtocol, desc: &DeviceDescriptor, index: u8) -> Result<()> {
    validate_profile_num(index, desc)?;
    proto.set_profile(index - 1)?;
    println!("Switched to profile {}", index);
    Ok(())
}

fn validate_profile_num(n: u8, desc: &DeviceDescriptor) -> Result<()> {
    if n < 1 || n > desc.num_profiles as u8 {
        anyhow::bail!("Profile must be 1-{}, got {}", desc.num_profiles, n);
    }
    Ok(())
}
