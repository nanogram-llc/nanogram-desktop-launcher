use std::path::Path;

pub fn extract_icon(exe_path: &str, output_path: &Path) -> bool {
    // For .app bundles, read the icon from Contents/Resources
    let app_path = Path::new(exe_path);
    let plist_path = app_path.join("Contents/Info.plist");

    let icon_name = if let Ok(plist_data) = plist::Value::from_file(&plist_path) {
        plist_data
            .as_dictionary()
            .and_then(|d| d.get("CFBundleIconFile"))
            .and_then(|v| v.as_string())
            .map(|s| s.to_string())
    } else {
        None
    };

    let icon_name = match icon_name {
        Some(n) => n,
        None => return false,
    };

    // Add .icns extension if missing
    let icon_file = if icon_name.ends_with(".icns") {
        icon_name
    } else {
        format!("{}.icns", icon_name)
    };

    let icon_path = app_path.join("Contents/Resources").join(&icon_file);
    if !icon_path.exists() {
        return false;
    }

    // Parse icns and extract the largest image
    let file = match std::fs::File::open(&icon_path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let icon_family = match icns::IconFamily::read(file) {
        Ok(f) => f,
        Err(_) => return false,
    };

    // Get the largest available icon
    let icon_types = [
        icns::IconType::RGBA32_512x512_2x,
        icns::IconType::RGBA32_512x512,
        icns::IconType::RGBA32_256x256_2x,
        icns::IconType::RGBA32_256x256,
        icns::IconType::RGBA32_128x128,
        icns::IconType::RGBA32_64x64,
        icns::IconType::RGBA32_32x32,
    ];

    for icon_type in &icon_types {
        if let Ok(img) = icon_family.get_icon_with_type(*icon_type) {
            let rgba_data = img.data().to_vec();
            let width = img.width();
            let height = img.height();

            if let Some(img_buf) =
                image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, rgba_data)
            {
                let resized = image::imageops::resize(
                    &img_buf,
                    64,
                    64,
                    image::imageops::FilterType::Lanczos3,
                );
                return resized.save(output_path).is_ok();
            }
        }
    }

    false
}
