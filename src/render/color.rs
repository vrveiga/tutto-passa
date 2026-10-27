//! Minimal ANSI styling: fg color, bold, underline, dim.

use crate::model::Hue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    None,
    Ansi256,
    TrueColor,
}

impl ColorMode {
    /// Color only on a TTY with `NO_COLOR` unset or empty.
    pub fn detect(stdout_is_tty: bool, no_color: Option<&str>, colorterm: Option<&str>) -> Self {
        if !stdout_is_tty || no_color.is_some_and(|v| !v.is_empty()) {
            ColorMode::None
        } else if matches!(colorterm, Some("truecolor" | "24bit")) {
            ColorMode::TrueColor
        } else {
            ColorMode::Ansi256
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

const fn hex(x: u32) -> Rgb {
    Rgb((x >> 16) as u8, (x >> 8) as u8, x as u8)
}

pub const MISSED: Rgb = hex(0x2d333b);
pub const SKIP: Rgb = hex(0x6e7681);

/// Levels 1–4 for each hue.
pub fn ramp(hue: Hue) -> [Rgb; 4] {
    let r = match hue {
        Hue::Green => [0x0e4429, 0x006d32, 0x26a641, 0x39d353],
        Hue::Blue => [0x0c2d6b, 0x1158c7, 0x388bfd, 0x79c0ff],
        Hue::Purple => [0x3c1e70, 0x6e40c9, 0xa371f7, 0xd2a8ff],
        Hue::Orange => [0x5a1e02, 0x9b4215, 0xdb6d28, 0xffa657],
        Hue::Red => [0x67060c, 0xa40e26, 0xf85149, 0xff7b72],
        Hue::Yellow => [0x4b3a03, 0x845e0b, 0xd29922, 0xf2cc60],
        Hue::Pink => [0x5e103e, 0x9e1f63, 0xdb61a2, 0xf778ba],
        Hue::Cyan => [0x0b3d45, 0x0f6e7a, 0x2bb3c0, 0x56d4dd],
    };
    r.map(hex)
}

/// Nearest xterm-256 index, from the 6×6×6 cube or the gray ramp.
pub fn xterm256(Rgb(r, g, b): Rgb) -> u8 {
    const LEVELS: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let near = |v: u8| {
        (0..6)
            .min_by_key(|&i| (LEVELS[i] - i32::from(v)).abs())
            .unwrap()
    };
    let (ri, gi, bi) = (near(r), near(g), near(b));
    let dist = |(x, y, z): (i32, i32, i32)| {
        (x - i32::from(r)).pow(2) + (y - i32::from(g)).pow(2) + (z - i32::from(b)).pow(2)
    };
    let cube = (LEVELS[ri], LEVELS[gi], LEVELS[bi]);
    let avg = (i32::from(r) + i32::from(g) + i32::from(b)) / 3;
    let gi_ = ((avg - 8 + 5) / 10).clamp(0, 23);
    let gray = 8 + 10 * gi_;
    if dist((gray, gray, gray)) < dist(cube) {
        232 + gi_ as u8
    } else {
        16 + 36 * ri as u8 + 6 * gi as u8 + bi as u8
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Style {
    pub fg: Option<Rgb>,
    pub bold: bool,
    pub dim: bool,
    pub underline: bool,
}

impl Style {
    pub fn fg(rgb: Rgb) -> Self {
        Style {
            fg: Some(rgb),
            ..Default::default()
        }
    }
    pub fn dim() -> Self {
        Style {
            dim: true,
            ..Default::default()
        }
    }
}

pub fn paint(mode: ColorMode, text: &str, style: Style) -> String {
    if mode == ColorMode::None {
        return text.to_string();
    }
    let mut codes = Vec::new();
    if style.bold {
        codes.push("1".to_string());
    }
    if style.dim {
        codes.push("2".to_string());
    }
    if style.underline {
        codes.push("4".to_string());
    }
    if let Some(c @ Rgb(r, g, b)) = style.fg {
        codes.push(match mode {
            ColorMode::TrueColor => format!("38;2;{r};{g};{b}"),
            _ => format!("38;5;{}", xterm256(c)),
        });
    }
    if codes.is_empty() {
        return text.to_string();
    }
    format!("\x1b[{}m{text}\x1b[0m", codes.join(";"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection() {
        use ColorMode as M;
        for (tty, nc, ct, want) in [
            (false, None, Some("truecolor"), M::None),
            (true, None, None, M::Ansi256),
            (true, Some(""), Some("24bit"), M::TrueColor),
            (true, Some("1"), Some("truecolor"), M::None),
            (true, None, Some("yes"), M::Ansi256),
        ] {
            assert_eq!(ColorMode::detect(tty, nc, ct), want);
        }
    }

    #[test]
    fn xterm_mapping() {
        assert_eq!(xterm256(Rgb(0, 0, 0)), 16);
        assert_eq!(xterm256(Rgb(255, 255, 255)), 231);
        assert_eq!(xterm256(Rgb(255, 0, 0)), 196);
        assert_eq!(xterm256(Rgb(128, 128, 128)), 244);
        assert_eq!(xterm256(hex(0x39d353)), 77);
    }

    #[test]
    fn painting() {
        assert_eq!(paint(ColorMode::None, "x", Style::fg(Rgb(1, 2, 3))), "x");
        assert_eq!(
            paint(ColorMode::TrueColor, "x", Style::fg(Rgb(1, 2, 3))),
            "\x1b[38;2;1;2;3mx\x1b[0m"
        );
        let s = Style {
            bold: true,
            underline: true,
            ..Style::fg(Rgb(255, 0, 0))
        };
        assert_eq!(
            paint(ColorMode::Ansi256, "x", s),
            "\x1b[1;4;38;5;196mx\x1b[0m"
        );
    }
}
