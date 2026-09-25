use jpeg_decoder::{Decoder, PixelFormat};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult, ErrorCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolvedCaptcha {
    pub left_digit: u8,
    pub operator: char,
    pub right_digit: u8,
    pub expression: String,
    pub answer: i32,
    pub confidence_distance: u32,
}

const DIGIT_TEMPLATES: &[(u8, u128)] = &[
    (0, 0x3e8c660d8360d8360d8360cc63f838_u128),
    (0, 0x3f8c660d8360d8360d8360cc63f838_u128),
    (1, 0x7f8300c0300c0300c0300c03e0f838_u128),
    (2, 0x3fc010180406010100c0300c10dc1c_u128),
    (2, 0x3fc030180c06030180c0300c11fc3e_u128),
    (3, 0x1fce1300c03807e0f860300c13fc3e_u128),
    (3, 0x1fce1300c03807e0f860300c11fc3e_u128),
    (4, 0x180607fdff198661b06c1e0701c060_u128),
    (5, 0x1fce1300c03807f07c0300c033fcff_u128),
    (5, 0x1fce1300803807f07c0300c033fcff_u128),
    (6, 0x3f9c660d8371cff1ec03018863f078_u128),
    (7, 0x3018060100c020180c0201807f9fe_u128),
    (8, 0x3f9c760d833987c1f04e318c63f87c_u128),
    (9, 0x1f8c2301806f1fe71d8360cc73f83c_u128),
    (9, 0x06042100804e0ee71582204c71d81c_u128),
];

const OP_TEMPLATES: &[(char, u128)] = &[
    ('+', 0x0802008020ffc20080200802000000_u128),
    ('*', 0x30cc31983c0f0180f03c198c330d81_u128),
    ('-', 0x0000000000ffc00000000000000000_u128),
];

pub struct SinopecCaptchaSolver;

impl SinopecCaptchaSolver {
    /// Decodes a `90x25` JPEG from `/corpgas/YanZhengMaServlet` and solves the arithmetic expression (`A + B = ?` or `A X B = ?`).
    pub fn solve_jpeg(jpeg_bytes: &[u8]) -> AppResult<SolvedCaptcha> {
        let mut decoder = Decoder::new(jpeg_bytes);
        let pixels = decoder.decode().map_err(|e| {
            AppError::new(
                ErrorCode::CaptchaRequired,
                format!("Failed to decode YanZhengMaServlet JPEG: {e}"),
            )
        })?;
        let info = decoder
            .info()
            .ok_or_else(|| AppError::new(ErrorCode::CaptchaRequired, "Missing JPEG metadata"))?;

        let width = info.width as usize;
        let height = info.height as usize;
        if width < 55 || height < 18 {
            return Err(AppError::new(
                ErrorCode::CaptchaRequired,
                format!("Unexpected captcha image size: {width}x{height}"),
            ));
        }

        let (d1, dist1) = Self::match_best_digit(&pixels, width, info.pixel_format, 10);
        let (op, dist_op) = Self::match_best_op(&pixels, width, info.pixel_format, 25);
        let (d2, dist2) = Self::match_best_digit(&pixels, width, info.pixel_format, 40);

        let answer = match op {
            '+' => (d1 as i32) + (d2 as i32),
            '*' => (d1 as i32) * (d2 as i32),
            '-' => (d1 as i32) - (d2 as i32),
            _ => (d1 as i32) + (d2 as i32),
        };

        let op_display = if op == '*' { '×' } else { op };
        Ok(SolvedCaptcha {
            left_digit: d1,
            operator: op,
            right_digit: d2,
            expression: format!("{d1} {op_display} {d2} = {answer}"),
            answer,
            confidence_distance: dist1 + dist_op + dist2,
        })
    }

    fn extract_patch_u128(
        pixels: &[u8],
        width: usize,
        format: PixelFormat,
        x_start: usize,
    ) -> u128 {
        let mut bits = 0_u128;
        let mut bit_idx = 0_u32;
        for y in 6..=17 {
            for x in x_start..(x_start + 10) {
                let lum_sum = match format {
                    PixelFormat::RGB24 => {
                        let idx = (y * width + x) * 3;
                        if idx + 2 < pixels.len() {
                            (pixels[idx] as u32)
                                + (pixels[idx + 1] as u32)
                                + (pixels[idx + 2] as u32)
                        } else {
                            765
                        }
                    }
                    PixelFormat::L8 => {
                        let idx = y * width + x;
                        pixels.get(idx).copied().unwrap_or(255) as u32 * 3
                    }
                    _ => 765,
                };
                if lum_sum < 360 {
                    bits |= 1_u128 << bit_idx;
                }
                bit_idx += 1;
            }
        }
        bits
    }

    fn match_best_digit(
        pixels: &[u8],
        width: usize,
        format: PixelFormat,
        x_base: usize,
    ) -> (u8, u32) {
        let mut best_digit = 0;
        let mut best_dist = u32::MAX;
        for dx in [-1_isize, 0, 1] {
            let x = (x_base as isize + dx).max(0) as usize;
            let mask = Self::extract_patch_u128(pixels, width, format, x);
            for &(digit, tmpl) in DIGIT_TEMPLATES {
                let dist = (mask ^ tmpl).count_ones();
                if dist < best_dist {
                    best_dist = dist;
                    best_digit = digit;
                }
            }
        }
        (best_digit, best_dist)
    }

    fn match_best_op(
        pixels: &[u8],
        width: usize,
        format: PixelFormat,
        x_base: usize,
    ) -> (char, u32) {
        let mut best_op = '+';
        let mut best_dist = u32::MAX;
        for dx in [-1_isize, 0, 1] {
            let x = (x_base as isize + dx).max(0) as usize;
            let mask = Self::extract_patch_u128(pixels, width, format, x);
            for &(op, tmpl) in OP_TEMPLATES {
                let dist = (mask ^ tmpl).count_ones();
                if dist < best_dist {
                    best_dist = dist;
                    best_op = op;
                }
            }
        }
        (best_op, best_dist)
    }
}
