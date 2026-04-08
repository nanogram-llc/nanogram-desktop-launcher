use image::{ImageBuffer, RgbaImage};
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, SelectObject, BITMAP,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows::Win32::UI::Shell::{
    ExtractIconExW, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON,
};
use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

pub fn extract_icon(exe_path: &str, output_path: &Path) -> bool {
    unsafe { extract_icon_inner(exe_path, output_path).unwrap_or(false) }
}

unsafe fn extract_icon_inner(
    exe_path: &str,
    output_path: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    let wide_path: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut large_icon = [windows::Win32::UI::WindowsAndMessaging::HICON::default()];

    let count = ExtractIconExW(
        PCWSTR(wide_path.as_ptr()),
        0,
        Some(large_icon.as_mut_ptr()),
        None,
        1,
    );

    if count == 0 || large_icon[0].is_invalid() {
        return Ok(false);
    }

    let hicon = large_icon[0];

    // Get icon info
    let mut icon_info = windows::Win32::UI::WindowsAndMessaging::ICONINFO::default();
    windows::Win32::UI::WindowsAndMessaging::GetIconInfo(hicon, &mut icon_info)?;

    // Get bitmap info
    let mut bmp = BITMAP::default();
    GetObjectW(
        icon_info.hbmColor,
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bmp as *mut _ as *mut _),
    );

    let width = bmp.bmWidth as u32;
    let height = bmp.bmHeight as u32;

    if width == 0 || height == 0 {
        let _ = DeleteObject(icon_info.hbmColor);
        let _ = DeleteObject(icon_info.hbmMask);
        DestroyIcon(hicon)?;
        return Ok(false);
    }

    // Create DC and extract bits
    let hdc = CreateCompatibleDC(None);
    let _old = SelectObject(hdc, icon_info.hbmColor);

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32), // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pixels: Vec<u8> = vec![0u8; (width * height * 4) as usize];

    GetDIBits(
        hdc,
        icon_info.hbmColor,
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );

    // BGRA to RGBA
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }

    // Save as PNG
    let img: RgbaImage = ImageBuffer::from_raw(width, height, pixels)
        .unwrap_or_else(|| ImageBuffer::new(width, height));

    // Resize to 64x64 for consistent display
    let resized = image::imageops::resize(&img, 64, 64, image::imageops::FilterType::Lanczos3);
    resized.save(output_path)?;

    // Cleanup
    let _ = DeleteDC(hdc);
    let _ = DeleteObject(icon_info.hbmColor);
    let _ = DeleteObject(icon_info.hbmMask);
    DestroyIcon(hicon)?;

    Ok(true)
}

/// Extract icon for any file or folder using SHGetFileInfoW.
pub fn extract_file_icon(path: &str, output_path: &Path) -> bool {
    unsafe { extract_file_icon_inner(path, output_path).unwrap_or(false) }
}

unsafe fn extract_file_icon_inner(
    path: &str,
    output_path: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut shfi = SHFILEINFOW::default();

    let result = SHGetFileInfoW(
        PCWSTR(wide_path.as_ptr()),
        windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
        Some(&mut shfi),
        std::mem::size_of::<SHFILEINFOW>() as u32,
        SHGFI_ICON | SHGFI_LARGEICON,
    );

    if result == 0 || shfi.hIcon.is_invalid() {
        return Ok(false);
    }

    let hicon = shfi.hIcon;

    // Get icon info
    let mut icon_info = windows::Win32::UI::WindowsAndMessaging::ICONINFO::default();
    windows::Win32::UI::WindowsAndMessaging::GetIconInfo(hicon, &mut icon_info)?;

    let mut bmp = BITMAP::default();
    GetObjectW(
        icon_info.hbmColor,
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bmp as *mut _ as *mut _),
    );

    let width = bmp.bmWidth as u32;
    let height = bmp.bmHeight as u32;

    if width == 0 || height == 0 {
        let _ = DeleteObject(icon_info.hbmColor);
        let _ = DeleteObject(icon_info.hbmMask);
        DestroyIcon(hicon)?;
        return Ok(false);
    }

    let hdc = CreateCompatibleDC(None);
    let _old = SelectObject(hdc, icon_info.hbmColor);

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pixels: Vec<u8> = vec![0u8; (width * height * 4) as usize];

    GetDIBits(
        hdc,
        icon_info.hbmColor,
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );

    // BGRA to RGBA
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }

    let img: RgbaImage = ImageBuffer::from_raw(width, height, pixels)
        .unwrap_or_else(|| ImageBuffer::new(width, height));
    let resized = image::imageops::resize(&img, 64, 64, image::imageops::FilterType::Lanczos3);
    resized.save(output_path)?;

    let _ = DeleteDC(hdc);
    let _ = DeleteObject(icon_info.hbmColor);
    let _ = DeleteObject(icon_info.hbmMask);
    DestroyIcon(hicon)?;

    Ok(true)
}
