use anyhow::Result;

use crate::devices::DeviceDescriptor;
use crate::razer::{self, RazerDevice};
use crate::types::{DeviceProfile, ProfileData};

use super::MouseProtocol;

/// Razer mouse protocol over hidraw feature reports.
pub struct RazerProtocol {
    pub dev: RazerDevice,
}

impl RazerProtocol {
    pub fn new(dev: RazerDevice) -> Self {
        Self { dev }
    }
}

impl MouseProtocol for RazerProtocol {
    fn name(&self) -> &str {
        "Razer"
    }

    fn get_profile_data(&mut self) -> Result<ProfileData> {
        // Razer mice don't have multi-profile firmware info in the same way.
        // Return a placeholder.
        Ok(ProfileData {
            current_profile: 0,
            firmware_version: "unknown".to_string(),
        })
    }

    fn read_current_profile(&mut self, desc: &DeviceDescriptor) -> Result<DeviceProfile> {
        razer::read_profile(&self.dev, desc)
    }

    fn read_all_profiles(&mut self, desc: &DeviceDescriptor) -> Result<(u8, Vec<DeviceProfile>)> {
        // Razer DeathAdder has a single profile
        let profile = self.read_current_profile(desc)?;
        Ok((0, vec![profile]))
    }

    fn apply_profile(&mut self, desc: &DeviceDescriptor, profile: &DeviceProfile) -> Result<()> {
        razer::apply_profile(&self.dev, desc, profile)
    }

    fn set_profile(&mut self, _index: u8) -> Result<()> {
        // Single profile, nothing to switch
        Ok(())
    }

    fn save(&mut self) -> Result<()> {
        // Razer settings persist automatically (stored in firmware)
        Ok(())
    }
}
