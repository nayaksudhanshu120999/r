use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    slint_build::compile("ui/palette.slint").expect("Failed to compile Slint UI");

    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        if let Ok(out_dir) = env::var("OUT_DIR") {
            let ico_path = Path::new(&out_dir).join("cmdpal.ico");
            if generate_app_ico(&ico_path).is_ok() {
                let mut res = winresource::WindowsResource::new();
                res.set_icon(ico_path.to_str().unwrap());
                let _ = res.compile();
            }
        }
    }
}

fn generate_app_ico(path: &Path) -> std::io::Result<()> {
    let size = 64u32;
    let mut bgra = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let (r, g, b, a) = render_icon_pixel(x, y, size);
            let flipped_y = size - 1 - y;
            let idx = ((flipped_y * size + x) * 4) as usize;
            bgra[idx] = b;
            bgra[idx + 1] = g;
            bgra[idx + 2] = r;
            bgra[idx + 3] = a;
        }
    }

    let and_mask_size = ((size * size) / 8) as usize;
    let and_mask = vec![0u8; and_mask_size];
    let image_size = (40 + bgra.len() + and_mask.len()) as u32;

    let mut file = File::create(path)?;

    // ICONDIR header (6 bytes)
    file.write_all(&0u16.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;

    // ICONDIRENTRY (16 bytes)
    file.write_all(&[size as u8, size as u8, 0, 0])?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&32u16.to_le_bytes())?;
    file.write_all(&image_size.to_le_bytes())?;
    file.write_all(&22u32.to_le_bytes())?;

    // BITMAPINFOHEADER (40 bytes)
    file.write_all(&40u32.to_le_bytes())?;
    file.write_all(&(size as i32).to_le_bytes())?;
    file.write_all(&((size * 2) as i32).to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&32u16.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;
    file.write_all(&(bgra.len() as u32).to_le_bytes())?;
    file.write_all(&0i32.to_le_bytes())?;
    file.write_all(&0i32.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;

    file.write_all(&bgra)?;
    file.write_all(&and_mask)?;
    Ok(())
}

fn render_icon_pixel(x: u32, y: u32, size: u32) -> (u8, u8, u8, u8) {
    let fx = x as f32 + 0.5;
    let fy = y as f32 + 0.5;
    let s = size as f32;
    let pad = s * 0.06;
    let radius = s * 0.22;

    let min_c = pad + radius;
    let max_c = s - pad - radius;
    let dx = if fx < min_c {
        min_c - fx
    } else if fx > max_c {
        fx - max_c
    } else {
        0.0
    };
    let dy = if fy < min_c {
        min_c - fy
    } else if fy > max_c {
        fy - max_c
    } else {
        0.0
    };
    let dist = (dx * dx + dy * dy).sqrt();
    if fx < pad || fx > s - pad || fy < pad || fy > s - pad || dist > radius {
        return (0, 0, 0, 0);
    }

    // Modern indigo-to-violet gradient
    let t = (fx + fy) / (2.0 * s);
    let mut r = (79.0 * (1.0 - t) + 124.0 * t) as u8;
    let mut g = (70.0 * (1.0 - t) + 58.0 * t) as u8;
    let mut b = (229.0 * (1.0 - t) + 237.0 * t) as u8;

    // Subtle border highlight
    if dist > radius - 1.4
        || fx < pad + 1.4
        || fx > s - pad - 1.4
        || fy < pad + 1.4
        || fy > s - pad - 1.4
    {
        r = r.saturating_add(35);
        g = g.saturating_add(35);
        b = b.saturating_add(25);
    }

    // Draw ">_" command prompt glyph
    let nx = fx / s;
    let ny = fy / s;

    let chevron_top =
        ny >= 0.30 && ny <= 0.52 && (nx - (0.26 + (ny - 0.30) * 0.95)).abs() < 0.065;
    let chevron_bot =
        ny > 0.52 && ny <= 0.74 && (nx - (0.47 - (ny - 0.52) * 0.95)).abs() < 0.065;
    let underscore = ny >= 0.66 && ny <= 0.75 && nx >= 0.52 && nx <= 0.76;

    if chevron_top || chevron_bot || underscore {
        return (255, 255, 255, 255);
    }

    (r, g, b, 255)
}
