# ratctl

Configure gaming mice on Linux. Buttons, DPI, LEDs, profiles — no daemon, no GUI toolkit, just a single binary.

Talks directly to mice over hidraw using vendor-specific HID protocols. Supports reading device state, writing config, and an interactive TUI.

## Supported devices

| Device | Vendor | Buttons | DPI | LEDs | Profiles | Polling rate |
|--------|--------|---------|-----|------|----------|--------------|
| ASUS ROG Spatha X | ASUS | Yes | Yes | Yes | 5 | Yes |
| Razer DeathAdder V2 | Razer | — | Yes | Yes | — | Yes |
| Corsair Scimitar RGB Elite | Corsair | — | Yes | Yes | — | Yes |

Adding a new device means implementing the wire protocol and adding a `DeviceDescriptor` to the registry. See [Adding devices](#adding-a-device) below.

### Limitations

- **Razer / Corsair button remapping** — not yet implemented. Buttons use their default hardware bindings.
- **Razer DPI presets** — only the active DPI is read/written. The DeathAdder V2 doesn't expose preset switching over HID, so config presets 2–5 are stored but have no effect on the device.
- **Corsair LED read** — the NXP protocol doesn't expose a clean GET for LED state in software mode. `ratctl dump` will report LEDs as black; `ratctl apply` will overwrite whatever the device had. Edit the dumped config before applying.
- **Corsair / Razer profiles** — single-profile only. The hardware may support multiple profiles but profile switching isn't implemented yet.
- **Razer debounce / angle snapping** — not configurable via the Razer HID protocol. Values in config are ignored.

## Installation

### From source

```sh
cargo install --path .
```

### udev rules

You need permission to access `/dev/hidraw*` and `/dev/input/event*` devices. Copy the included rules file:

```sh
sudo cp udev/99-ratctl.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Then log out and back in (or reboot) for `uaccess` tags to take effect.

## Usage

```
ratctl [OPTIONS] <COMMAND>

Commands:
  status   Print current device settings
  dump     Read device state and write to config file
  apply    Apply config file to device
  profile  Switch active profile on device
  tui      Interactive TUI configurator
  devices  List supported devices
```

### Quick start

```sh
# See what's currently configured
ratctl status

# Dump device state to config file (~/.config/ratctl/config.toml)
ratctl dump

# Edit the config, then apply it back
ratctl apply

# Or use the interactive TUI
ratctl tui
```

### Options

- `-c, --config <PATH>` — Path to config file (default: `~/.config/ratctl/config.toml`)
- `-d, --device <PATH>` — Path to hidraw device (auto-detected if omitted)

### TUI keybindings

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Switch tabs (Buttons, DPI, LEDs, Settings) |
| `1`-`5` | Switch profile |
| `j`/`k` or arrows | Move cursor |
| `Enter` | Edit selected item |
| `<` / `>` | Adjust value inline (DPI, polling rate, debounce, LED mode) |
| `p` | Press a physical button to select it (Buttons tab) |
| `c` | Edit LED color as hex (LEDs tab) |
| `b` | Cycle LED brightness (LEDs tab) |
| `s` | Save config to file |
| `a` | Apply all changes to device |
| `q` | Quit (warns on unsaved changes) |
| `Ctrl+C` | Force quit |

## Config format

Config is TOML. A `ratctl dump` produces something like:

```toml
active_profile = 1

[profile.1]
polling_rate = 1000
debounce_ms = 8
angle_snapping = false

[profile.1.dpi]
preset_1 = 800
preset_2 = 1600
preset_3 = 3200
preset_4 = 6400

[profile.1.buttons]
left_click = "left_click"
right_click = "right_click"
middle_click = "middle_click"
back = "back"
forward = "forward"
dpi_cycle = "dpi_cycle"
scroll_up = "key:f5"
side_a = "disabled"

[profile.1.leds.logo]
mode = "static"
brightness = 4
color = "#ff0000"

[profile.1.leds.wheel]
mode = "breathing"
brightness = 3
color = "#00ff00"
```

### Button bindings

- Mouse actions: `left_click`, `right_click`, `middle_click`, `back`, `forward`, `dpi_cycle`, `dpi_target`, `scroll_up`, `scroll_down`, `side_a` through `side_l`, `disabled`
- Keyboard keys: `key:<name>` — e.g. `key:f5`, `key:space`, `key:a`

### LED modes

`static`, `breathing`, `cycle`, `rainbow`, `reactive`, `custom`, `battery`

## Adding a device

1. Add a `DeviceDescriptor` to `src/devices.rs` with VID/PID, button layout, LED zones, and DPI range
2. Implement the wire protocol in a new module (e.g. `src/steelseries.rs`)
3. Wrap it in a `MouseProtocol` impl under `src/protocols/`
4. Add the device to `open_protocol()` in `src/main.rs`
5. Add udev rules to `udev/99-ratctl.rules`

Look at `src/razer.rs` and `src/protocols/razer.rs` for a minimal example.

## License

MIT
