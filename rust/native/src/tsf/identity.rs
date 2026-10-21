// SPDX-License-Identifier: GPL-3.0-or-later
//! Product-owned identities. Never reuse upstream Meltype's COM/profile IDs.
use windows::core::GUID;
pub(super) const CLSID: GUID = GUID::from_u128(0xc540baf6_88b1_5b30_b524_2c8702b034fc);
pub(super) const PROFILE: GUID = GUID::from_u128(0x6d01de9c_8ab3_5cfc_a5c6_10893cdf8762);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flash_registration_is_distinct_from_upstream_and_matches_management_scripts() {
        assert_ne!(
            CLSID,
            GUID::from_u128(0x417d801b_a9bd_4c26_bd16_356a825a6998)
        );
        assert_ne!(
            PROFILE,
            GUID::from_u128(0x21f643f4_72d5_4946_bf70_0a126ab52d05)
        );
        let manager = include_str!("../../../Manage-Windows.ps1");
        let acceptance = include_str!("../../../Test-InstalledIme.ps1");
        assert!(!manager.contains("417D801B-A9BD-4C26-BD16-356A825A6998"));
        for source in [manager, acceptance] {
            assert!(source.contains("C540BAF6-88B1-5B30-B524-2C8702B034FC"));
        }
    }
}
