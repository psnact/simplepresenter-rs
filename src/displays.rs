//! Connected monitors for the "Projection Display" setting.

pub struct Display {
    pub label: String,
    /// Top-left corner in logical points.
    pub pos: [f32; 2],
    /// Size in logical points.
    pub size: [f32; 2],
}

#[cfg(any(windows, target_os = "macos"))]
pub fn list() -> Vec<Display> {
    let all = display_info::DisplayInfo::all().unwrap_or_default();
    all.iter()
        .enumerate()
        .map(|(i, d)| {
            let scale = if d.scale_factor.is_finite() && d.scale_factor > 0.0 {
                d.scale_factor
            } else {
                1.0
            };
            let mut label = format!("Display {}", i + 1);
            if d.is_primary {
                label += " (Primary - Current App)";
            }
            label += &format!(" - {}x{}", d.width, d.height);
            Display {
                label,
                pos: [d.x as f32 / scale, d.y as f32 / scale],
                size: [d.width as f32 / scale, d.height as f32 / scale],
            }
        })
        .collect()
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn list() -> Vec<Display> {
    Vec::new()
}
