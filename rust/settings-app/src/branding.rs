//! Shared product mark for the root window and native child windows.
use eframe::egui::IconData;
use std::sync::{Arc, OnceLock};

pub fn icon() -> Arc<IconData> {
    static ICON: OnceLock<Arc<IconData>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(
            eframe::icon_data::from_png_bytes(include_bytes!(
                "../../../assets/branding/meltype-flash-256.png"
            ))
            .expect("embedded Meltype-flash icon must be valid PNG"),
        )
    })
    .clone()
}
