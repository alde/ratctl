use anyhow::Result;

use crate::device::HidrawDevice;
use crate::devices::DeviceDescriptor;
use crate::protocol;
use crate::types::{DeviceProfile, ProfileData};

use super::MouseProtocol;

/// ASUS ROG mouse protocol over hidraw.
pub struct AsusProtocol {
    pub dev: HidrawDevice,
}

impl AsusProtocol {
    pub fn new(dev: HidrawDevice) -> Self {
        Self { dev }
    }
}

impl MouseProtocol for AsusProtocol {
    fn name(&self) -> &str {
        "ASUS ROG"
    }

    fn get_profile_data(&mut self) -> Result<ProfileData> {
        protocol::get_profile_data(&mut self.dev)
    }

    fn read_current_profile(&mut self, desc: &DeviceDescriptor) -> Result<DeviceProfile> {
        protocol::read_current_profile(&mut self.dev, desc)
    }

    fn read_all_profiles(&mut self, desc: &DeviceDescriptor) -> Result<(u8, Vec<DeviceProfile>)> {
        protocol::read_all_profiles(&mut self.dev, desc)
    }

    fn apply_profile(&mut self, desc: &DeviceDescriptor, profile: &DeviceProfile) -> Result<()> {
        protocol::apply_profile(&mut self.dev, desc, profile)
    }

    fn set_profile(&mut self, index: u8) -> Result<()> {
        protocol::set_profile(&mut self.dev, index)
    }

    fn save(&mut self) -> Result<()> {
        protocol::save(&mut self.dev)
    }
}
