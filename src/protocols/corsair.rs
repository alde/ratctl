use anyhow::Result;

use crate::corsair::{self, CorsairDevice};
use crate::devices::DeviceDescriptor;
use crate::types::{DeviceProfile, ProfileData};

use super::MouseProtocol;

/// Corsair NXP mouse protocol over hidraw.
pub struct CorsairProtocol {
    pub dev: CorsairDevice,
}

impl CorsairProtocol {
    pub fn new(dev: CorsairDevice) -> Self {
        Self { dev }
    }
}

impl MouseProtocol for CorsairProtocol {
    fn name(&self) -> &str {
        "Corsair"
    }

    fn get_profile_data(&mut self) -> Result<ProfileData> {
        Ok(ProfileData {
            current_profile: 0,
            firmware_version: "unknown".to_string(),
        })
    }

    fn read_current_profile(&mut self, desc: &DeviceDescriptor) -> Result<DeviceProfile> {
        corsair::read_profile(&mut self.dev, desc)
    }

    fn read_all_profiles(&mut self, desc: &DeviceDescriptor) -> Result<(u8, Vec<DeviceProfile>)> {
        let profile = self.read_current_profile(desc)?;
        Ok((0, vec![profile]))
    }

    fn apply_profile(&mut self, desc: &DeviceDescriptor, profile: &DeviceProfile) -> Result<()> {
        corsair::apply_profile(&mut self.dev, desc, profile)
    }

    fn set_profile(&mut self, _index: u8) -> Result<()> {
        Ok(())
    }

    fn save(&mut self) -> Result<()> {
        // NXP devices persist settings when written with profile=0x01
        Ok(())
    }
}
