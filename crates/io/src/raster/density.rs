//! Bounded physical-resolution metadata readers. Invalid or unitless values are absent.

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at.checked_add(2)?)?.try_into().ok()?))
}
fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

pub(super) fn read(b: &[u8]) -> Option<[f64; 2]> {
    let value = if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        let mut at = 8usize;
        let mut found = None;
        for _ in 0..4096 {
            let len = usize::try_from(be32(b, at)?).ok()?;
            let data = at.checked_add(8)?;
            let end = data.checked_add(len)?;
            let kind = b.get(at.checked_add(4)?..data)?;
            let payload = b.get(data..end)?;
            if kind == b"pHYs" && payload.len() == 9 && payload.get(8) == Some(&1) {
                found = Some([f64::from(be32(payload, 0)?) * 0.0254, f64::from(be32(payload, 4)?) * 0.0254]);
                break;
            }
            if kind == b"IDAT" || kind == b"IEND" {
                break;
            }
            at = end.checked_add(4)?;
        }
        found
    } else if b.starts_with(b"BM") {
        if u32::from_le_bytes(b.get(14..18)?.try_into().ok()?) < 40 {
            return None;
        }
        let x = i32::from_le_bytes(b.get(38..42)?.try_into().ok()?);
        let y = i32::from_le_bytes(b.get(42..46)?.try_into().ok()?);
        Some([f64::from(x) * 0.0254, f64::from(y) * 0.0254])
    } else if b.starts_with(&[255, 216]) {
        let mut at = 2usize;
        let mut found = None;
        for _ in 0..4096 {
            if b.get(at) != Some(&255) {
                break;
            }
            while b.get(at) == Some(&255) {
                at = at.checked_add(1)?;
            }
            let marker = *b.get(at)?;
            at = at.checked_add(1)?;
            if marker == 218 || marker == 217 {
                break;
            }
            if marker == 1 || (208..=215).contains(&marker) {
                continue;
            }
            let len = usize::from(be16(b, at)?);
            if len < 2 {
                return None;
            }
            let end = at.checked_add(len)?;
            let p = b.get(at.checked_add(2)?..end)?;
            if marker == 224 && p.starts_with(b"JFIF\0") {
                let factor = match p.get(7)? {
                    1 => 1.0,
                    2 => 2.54,
                    _ => 0.0,
                };
                if factor > 0.0 {
                    found = Some([f64::from(be16(p, 8)?) * factor, f64::from(be16(p, 10)?) * factor]);
                }
            }
            if marker == 225 && p.starts_with(b"Exif\0\0") {
                if let Some(dpi) = tiff(p.get(6..)?) {
                    return Some(dpi);
                }
            }
            at = end;
        }
        found
    } else {
        tiff(b)
    };
    value.filter(|v| v.iter().all(|n| n.is_finite() && *n > 0.0 && *n <= 1e6))
}

fn tiff(b: &[u8]) -> Option<[f64; 2]> {
    let little = match b.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |at: usize| -> Option<u16> {
        let a = b.get(at..at.checked_add(2)?)?.try_into().ok()?;
        Some(if little { u16::from_le_bytes(a) } else { u16::from_be_bytes(a) })
    };
    let u32_at = |at: usize| -> Option<u32> {
        let a = b.get(at..at.checked_add(4)?)?.try_into().ok()?;
        Some(if little { u32::from_le_bytes(a) } else { u32::from_be_bytes(a) })
    };
    if u16_at(2)? != 42 {
        return None;
    }
    let ifd = usize::try_from(u32_at(4)?).ok()?;
    let count = usize::from(u16_at(ifd)?).min(4096);
    let (mut x, mut y, mut unit) = (None, None, 2);
    for i in 0..count {
        let at = ifd.checked_add(2)?.checked_add(i.checked_mul(12)?)?;
        let tag = u16_at(at)?;
        if u32_at(at.checked_add(4)?)? != 1 {
            continue;
        }
        if tag == 296 && u16_at(at + 2)? == 3 {
            unit = u16_at(at + 8)?;
        }
        if (tag == 282 || tag == 283) && u16_at(at + 2)? == 5 {
            let offset = usize::try_from(u32_at(at + 8)?).ok()?;
            let value = f64::from(u32_at(offset)?) / f64::from(u32_at(offset.checked_add(4)?)?);
            if tag == 282 {
                x = Some(value);
            } else {
                y = Some(value);
            }
        }
    }
    let factor = match unit {
        2 => 1.0,
        3 => 2.54,
        _ => return None,
    };
    let dpi = [x? * factor, y? * factor];
    dpi.iter().all(|n| n.is_finite() && *n > 0.0 && *n <= 1e6).then_some(dpi)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_density_and_truncation() {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(&9u32.to_be_bytes());
        png.extend_from_slice(b"pHYs");
        png.extend_from_slice(&11811u32.to_be_bytes());
        png.extend_from_slice(&11811u32.to_be_bytes());
        png.push(1);
        let dpi = read(&png).unwrap();
        assert!((dpi[0] - 300.0).abs() < 0.01);
        for n in 0..png.len() {
            assert!(read(&png[..n]).is_none());
        }
        *png.last_mut().unwrap() = 0;
        assert!(read(&png).is_none());
        let jpeg = [255, 216, 255, 224, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 2, 1, 1, 44, 1, 44, 0, 0, 255, 217];
        assert_eq!(read(&jpeg), Some([300.0, 300.0]));
        let mut bmp = vec![0u8; 46];
        bmp[..2].copy_from_slice(b"BM");
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[38..42].copy_from_slice(&11811u32.to_le_bytes());
        bmp[42..46].copy_from_slice(&11811u32.to_le_bytes());
        assert!((read(&bmp).unwrap()[0] - 300.0).abs() < 0.01);
    }
    #[test]
    fn tiff_and_exif_resolution() {
        let mut b = b"II\x2a\0\x08\0\0\0".to_vec();
        b.extend_from_slice(&3u16.to_le_bytes());
        for (tag, kind, value) in [(282u16, 5u16, 50u32), (283, 5, 58), (296, 3, 2)] {
            b.extend_from_slice(&tag.to_le_bytes());
            b.extend_from_slice(&kind.to_le_bytes());
            b.extend_from_slice(&1u32.to_le_bytes());
            b.extend_from_slice(&value.to_le_bytes());
        }
        b.extend_from_slice(&0u32.to_le_bytes());
        for _ in 0..2 {
            b.extend_from_slice(&300u32.to_le_bytes());
            b.extend_from_slice(&1u32.to_le_bytes());
        }
        assert_eq!(read(&b), Some([300.0, 300.0]));
        for n in 0..b.len() {
            assert!(read(&b[..n]).is_none());
        }
        let mut jpeg = vec![255, 216, 255, 225];
        jpeg.extend_from_slice(&u16::try_from(b.len() + 8).unwrap().to_be_bytes());
        jpeg.extend_from_slice(b"Exif\0\0");
        jpeg.extend_from_slice(&b);
        assert_eq!(read(&jpeg), Some([300.0, 300.0]));
    }
}
