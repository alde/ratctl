pub mod asus;
pub mod corsair;
pub mod razer;

use anyhow::Result;

use crate::devices::DeviceDescriptor;
use crate::types::{DeviceProfile, ProfileData};

/// Trait for communicating with a mouse over its vendor-specific protocol.
/// Implementations handle the wire format; callers work with `DeviceProfile`.
pub trait MouseProtocol {
    /// Human-readable protocol name.
    fn name(&self) -> &str;

    /// Read firmware version and current profile index.
    fn get_profile_data(&mut self) -> Result<ProfileData>;

    /// Read the complete state of the currently active profile.
    fn read_current_profile(&mut self, desc: &DeviceDescriptor) -> Result<DeviceProfile>;

    /// Read all profiles (switches profiles internally, restores original).
    fn read_all_profiles(&mut self, desc: &DeviceDescriptor) -> Result<(u8, Vec<DeviceProfile>)>;

    /// Write a complete profile to the currently active slot.
    fn apply_profile(&mut self, desc: &DeviceDescriptor, profile: &DeviceProfile) -> Result<()>;

    /// Switch the active profile on the device.
    fn set_profile(&mut self, index: u8) -> Result<()>;

    /// Persist current settings to device flash.
    fn save(&mut self) -> Result<()>;
}
