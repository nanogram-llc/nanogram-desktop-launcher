use image::{ImageBuffer, RgbaImage};
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, SelectObject, BITMAP,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::UI::Shell::{
    ExtractIconExW, IShellItemImageFactory, SHCreateItemFromParsingName, SHGetFileInfoW,
    SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};
use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

pub fn extract_icon(exe_path: &str, output_path: &Path) -> bool {
    // 1. ExtractIconExW: highest quality, reads embedded icon resources.
    //    Fails for EXEs without resources (some portable apps, console tools).
    if unsafe { extract_icon_inner(exe_path, output_path).unwrap_or(false) } {
        return true;
    }
    // 2. PE resource direct extraction. Reassembles the embedded
    //    RT_GROUP_ICON + RT_ICON entries into a standalone .ico in memory
    //    and decodes it. Catches old EXEs whose icons confuse the shell
    //    APIs but still parse cleanly from the PE.
    if extract_via_pe_resources(exe_path, output_path) {
        return true;
    }
    // 3. IShellItemImageFactory: the modern Explorer-style API. Resolves
    //    .lnk IconLocation, UWP / MSIX packages, file-association icons,
    //    and even document thumbnails.
    if unsafe { extract_via_shell_image_factory(exe_path, output_path).unwrap_or(false) } {
        return true;
    }
    // 4. SHGetFileInfoW: classic shell info — last-ditch generic icon
    //    (better than nothing).
    extract_file_icon(exe_path, output_path)
}

/// Extract icons by parsing RT_GROUP_ICON / RT_ICON resources directly from
/// the PE file, then re-assembling them into a real .ico in memory. The
/// image crate's `ico` feature decodes both classic DIB-based icons and the
/// newer PNG-encoded icons.
fn extract_via_pe_resources(exe_path: &str, output_path: &Path) -> bool {
    let bytes = match std::fs::read(exe_path) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let pe = match pelite::PeFile::from_bytes(&bytes) {
        Ok(p) => p,
        Err(_) => return false,
    };

    let resources = match pe.resources() {
        Ok(r) => r,
        Err(_) => return false,
    };

    let mut best: Option<(u32, image::DynamicImage)> = None;

    for entry in resources.icons().filter_map(Result::ok) {
        let (_, group) = entry;
        let mut ico_bytes: Vec<u8> = Vec::new();
        if group.write(&mut ico_bytes).is_err() {
            continue;
        }
        let img = match image::load_from_memory_with_format(
            &ico_bytes,
            image::ImageFormat::Ico,
        ) {
            Ok(i) => i,
            Err(_) => continue,
        };
        let area = img.width().saturating_mul(img.height());
        if best.as_ref().map(|(a, _)| area > *a).unwrap_or(true) {
            best = Some((area, img));
        }
    }

    let img = match best {
        Some((_, img)) => img,
        None => return false,
    };

    let rgba = img.to_rgba8();
    let resized =
        image::imageops::resize(&rgba, 64, 64, image::imageops::FilterType::Lanczos3);
    resized.save(output_path).is_ok()
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
    let result = hicon_to_rgba(hicon);
    DestroyIcon(hicon)?;
    let (width, height, pixels) = match result {
        Some(t) => t,
        None => return Ok(false),
    };
    Ok(save_rgba_as_png(width, height, pixels, output_path))
}

/// Read an HICON's color + mask bitmaps and combine them into 32-bit RGBA.
/// Old icons (24-bit color + 1-bit mask) ship without an alpha channel —
/// we derive alpha from the AND mask. Modern 32-bit icons keep their alpha.
unsafe fn hicon_to_rgba(
    hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
) -> Option<(u32, u32, Vec<u8>)> {
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    let mut info = ICONINFO::default();
    if GetIconInfo(hicon, &mut info).is_err() {
        return None;
    }

    let mut bm_color = BITMAP::default();
    if GetObjectW(
        info.hbmColor,
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm_color as *mut _ as *mut _),
    ) == 0
    {
        let _ = DeleteObject(info.hbmColor);
        let _ = DeleteObject(info.hbmMask);
        return None;
    }
    let width = bm_color.bmWidth as u32;
    let height = bm_color.bmHeight as u32;
    if width == 0 || height == 0 {
        let _ = DeleteObject(info.hbmColor);
        let _ = DeleteObject(info.hbmMask);
        return None;
    }

    let hdc = CreateCompatibleDC(None);

    let make_bmi = || BITMAPINFO {
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

    // Color bitmap as 32-bit BGRA.
    let mut bmi_color = make_bmi();
    let mut pixels: Vec<u8> = vec![0u8; (width * height * 4) as usize];
    let scanlines = GetDIBits(
        hdc,
        info.hbmColor,
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi_color,
        DIB_RGB_COLORS,
    );
    if scanlines == 0 {
        let _ = DeleteDC(hdc);
        let _ = DeleteObject(info.hbmColor);
        let _ = DeleteObject(info.hbmMask);
        return None;
    }

    // BGRA → RGBA, while watching for an all-zero alpha channel which
    // signals an old-style 24-bit color bitmap that needs the mask.
    let mut has_alpha = false;
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
        if chunk[3] != 0 {
            has_alpha = true;
        }
    }

    if !has_alpha {
        // Read the AND mask as a 32-bit DIB so we can index it the same way.
        // In the resulting bytes, mask=white means transparent and
        // mask=black means opaque.
        let mut bmi_mask = make_bmi();
        let mut mask: Vec<u8> = vec![0u8; (width * height * 4) as usize];
        let mask_lines = GetDIBits(
            hdc,
            info.hbmMask,
            0,
            height,
            Some(mask.as_mut_ptr() as *mut _),
            &mut bmi_mask,
            DIB_RGB_COLORS,
        );
        if mask_lines > 0 {
            for (color_chunk, mask_chunk) in
                pixels.chunks_exact_mut(4).zip(mask.chunks_exact(4))
            {
                // White (0xFF) in mask = transparent; black (0x00) = opaque.
                let transparent = mask_chunk[0] != 0;
                color_chunk[3] = if transparent { 0 } else { 0xFF };
            }
        } else {
            // Mask read failed — fall back to fully opaque so the icon
            // is at least visible.
            for chunk in pixels.chunks_exact_mut(4) {
                chunk[3] = 0xFF;
            }
        }
    }

    let _ = DeleteDC(hdc);
    let _ = DeleteObject(info.hbmColor);
    let _ = DeleteObject(info.hbmMask);

    Some((width, height, pixels))
}

/// Extract icon for any file or folder using SHGetFileInfoW.
pub fn extract_file_icon(path: &str, output_path: &Path) -> bool {
    if unsafe { extract_file_icon_inner(path, output_path).unwrap_or(false) } {
        return true;
    }
    // Final fallback: try the modern shell image factory before giving up.
    unsafe { extract_via_shell_image_factory(path, output_path).unwrap_or(false) }
}

/// Convert an HBITMAP (typically a 32-bit ARGB DIB returned by
/// IShellItemImageFactory) to RGBA pixel bytes plus dimensions.
unsafe fn hbitmap_to_rgba(hbitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut bmp = BITMAP::default();
    if GetObjectW(
        hbitmap,
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bmp as *mut _ as *mut _),
    ) == 0
    {
        return None;
    }
    let width = bmp.bmWidth as u32;
    let height = bmp.bmHeight as u32;
    if width == 0 || height == 0 {
        return None;
    }

    let hdc = CreateCompatibleDC(None);
    let _old = SelectObject(hdc, hbitmap);

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

    let scanlines = GetDIBits(
        hdc,
        hbitmap,
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );

    let _ = DeleteDC(hdc);

    if scanlines == 0 {
        return None;
    }

    // BGRA → RGBA.
    let mut all_zero_alpha = true;
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
        if chunk[3] != 0 {
            all_zero_alpha = false;
        }
    }
    // If the entire image is fully transparent, the bitmap was an opaque
    // 32-bit DIB without a real alpha channel — fix it up.
    if all_zero_alpha {
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[3] = 0xFF;
        }
    }

    Some((width, height, pixels))
}

/// Save a (width, height, RGBA bytes) tuple as a 64×64 PNG to `output_path`.
fn save_rgba_as_png(width: u32, height: u32, pixels: Vec<u8>, output_path: &Path) -> bool {
    let img: RgbaImage = match ImageBuffer::from_raw(width, height, pixels) {
        Some(img) => img,
        None => return false,
    };
    let resized = image::imageops::resize(&img, 64, 64, image::imageops::FilterType::Lanczos3);
    resized.save(output_path).is_ok()
}

/// Modern fallback using the shell's IShellItemImageFactory. This is what
/// File Explorer uses — it handles .lnk IconLocation, UWP/MSIX apps,
/// file-association icons, and document thumbnails.
unsafe fn extract_via_shell_image_factory(
    path: &str,
    output_path: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let factory: IShellItemImageFactory =
        match SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None) {
            Ok(f) => f,
            Err(_) => return Ok(false),
        };

    let size = SIZE { cx: 256, cy: 256 };
    // ICONONLY: never use a thumbnail/preview, always force a real icon —
    //   without this, GetImage may return E_PENDING for files that have
    //   thumbnail providers but no cached thumbnail.
    // BIGGERSIZEOK: accept whatever the largest available is, we resize later.
    let flags = SIIGBF_BIGGERSIZEOK | SIIGBF_ICONONLY;
    let hbitmap = match factory.GetImage(size, flags) {
        Ok(b) => b,
        Err(_) => return Ok(false),
    };

    if hbitmap.is_invalid() {
        return Ok(false);
    }

    let rgba = hbitmap_to_rgba(hbitmap);
    let _ = DeleteObject(hbitmap);

    let (w, h, pixels) = match rgba {
        Some(t) => t,
        None => return Ok(false),
    };

    Ok(save_rgba_as_png(w, h, pixels, output_path))
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
    let rgba = hicon_to_rgba(hicon);
    DestroyIcon(hicon)?;
    let (w, h, pixels) = match rgba {
        Some(t) => t,
        None => return Ok(false),
    };
    Ok(save_rgba_as_png(w, h, pixels, output_path))
}
