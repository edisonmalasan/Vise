/// Greatest common divisor for non-negative integers.
pub fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Reduce a width/height pair to its simplest integer ratio.
pub fn reduce_ratio(width: u32, height: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (width, height);
    }
    let d = gcd(width, height);
    (width / d, height / d)
}

/// Approximate a positive real value by the closest (width, height) integer
/// ratio with denominator at most `max_den`.
fn best_rational(value: f64, max_den: u32) -> (u32, u32) {
    if !value.is_finite() || value <= 0.0 {
        return (0, 0);
    }
    let mut best: (u32, u32) = (1, 1);
    let mut best_err = f64::INFINITY;
    for q in 1..=max_den {
        let p = (value * (q as f64)).round() as u32;
        if p == 0 {
            continue;
        }
        let err = ((p as f64) / (q as f64) - value).abs();
        if err < best_err {
            best_err = err;
            best = (p, q);
            if err == 0.0 {
                break;
            }
        }
    }
    reduce_ratio(best.0, best.1)
}

/// Compute the display aspect ratio for given storage dimensions and pixel
/// aspect ratio. Returns a reduced (width, height) integer pair.
pub fn display_aspect_ratio(
    storage_width: u32,
    storage_height: u32,
    pixel_aspect: f64,
) -> (u32, u32) {
    if storage_width == 0 || storage_height == 0 || pixel_aspect <= 0.0 || !pixel_aspect.is_finite()
    {
        return (storage_width, storage_height);
    }
    let ratio = (storage_width as f64) * pixel_aspect / (storage_height as f64);
    best_rational(ratio, 10_000)
}

/// Scale source dimensions to fit within a maximum box while preserving aspect
/// ratio. Returns the largest (width, height) that fits.
pub fn scale_to_fit(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 || max_w == 0 || max_h == 0 {
        return (0, 0);
    }
    let scale_w = max_w as f64 / src_w as f64;
    let scale_h = max_h as f64 / src_h as f64;
    let scale = scale_w.min(scale_h);
    let out_w = ((src_w as f64) * scale).round() as u32;
    let out_h = ((src_h as f64) * scale).round() as u32;
    let out_w = out_w.min(max_w);
    let out_h = out_h.min(max_h);
    (out_w.max(1), out_h.max(1))
}

/// Apply a pixel aspect ratio to storage dimensions, producing display pixel
/// dimensions rounded to the nearest integer.
pub fn apply_pixel_aspect(
    storage_width: u32,
    storage_height: u32,
    pixel_aspect: f64,
) -> (u32, u32) {
    if storage_width == 0 || storage_height == 0 || pixel_aspect <= 0.0 || !pixel_aspect.is_finite()
    {
        return (storage_width, storage_height);
    }
    let w = ((storage_width as f64) * pixel_aspect).round() as u32;
    (w.max(1), storage_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcd_basic() {
        assert_eq!(gcd(48, 18), 6);
        assert_eq!(gcd(0, 5), 5);
        assert_eq!(gcd(7, 0), 7);
    }

    #[test]
    fn reduce_ratio_basic() {
        assert_eq!(reduce_ratio(1920, 1080), (16, 9));
        assert_eq!(reduce_ratio(0, 0), (0, 0));
    }

    #[test]
    fn display_aspect_square_par() {
        assert_eq!(display_aspect_ratio(1920, 1080, 1.0), (16, 9));
    }

    #[test]
    fn display_aspect_nonsquare_par() {
        let (w, h) = display_aspect_ratio(720, 480, 32.0 / 27.0);
        assert_eq!((w, h), (16, 9));
    }

    #[test]
    fn scale_to_fit_inside_box() {
        assert_eq!(scale_to_fit(1920, 1080, 1280, 720), (1280, 720));
        assert_eq!(scale_to_fit(1920, 1080, 640, 640), (640, 360));
    }

    #[test]
    fn apply_pixel_aspect_rounds() {
        assert_eq!(apply_pixel_aspect(720, 480, 1.0), (720, 480));
    }
}
