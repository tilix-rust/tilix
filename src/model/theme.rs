use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ThemeError {
    #[error("Invalid hex color format: {0}")]
    InvalidHex(String),
    #[error("Palette must contain exactly 16 colors, found {0}")]
    InvalidPaletteLength(usize),
    #[error("JSON serialization error: {0}")]
    JsonError(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RgbColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl RgbColor {
    pub fn new(red: f64, green: f64, blue: f64, alpha: f64) -> Self {
        Self {
            red: red.clamp(0.0, 1.0),
            green: green.clamp(0.0, 1.0),
            blue: blue.clamp(0.0, 1.0),
            alpha: alpha.clamp(0.0, 1.0),
        }
    }

    pub fn rgb(red: f64, green: f64, blue: f64) -> Self {
        Self::new(red, green, blue, 1.0)
    }

    pub fn from_hex(hex: &str) -> Result<Self, ThemeError> {
        let clean = hex.trim().trim_start_matches('#');
        match clean.len() {
            6 => {
                let r = u8::from_str_radix(&clean[0..2], 16)
                    .map_err(|_| ThemeError::InvalidHex(hex.to_string()))?;
                let g = u8::from_str_radix(&clean[2..4], 16)
                    .map_err(|_| ThemeError::InvalidHex(hex.to_string()))?;
                let b = u8::from_str_radix(&clean[4..6], 16)
                    .map_err(|_| ThemeError::InvalidHex(hex.to_string()))?;
                Ok(Self {
                    red: r as f64 / 255.0,
                    green: g as f64 / 255.0,
                    blue: b as f64 / 255.0,
                    alpha: 1.0,
                })
            }
            8 => {
                let a = u8::from_str_radix(&clean[0..2], 16)
                    .map_err(|_| ThemeError::InvalidHex(hex.to_string()))?;
                let r = u8::from_str_radix(&clean[2..4], 16)
                    .map_err(|_| ThemeError::InvalidHex(hex.to_string()))?;
                let g = u8::from_str_radix(&clean[4..6], 16)
                    .map_err(|_| ThemeError::InvalidHex(hex.to_string()))?;
                let b = u8::from_str_radix(&clean[6..8], 16)
                    .map_err(|_| ThemeError::InvalidHex(hex.to_string()))?;
                Ok(Self {
                    red: r as f64 / 255.0,
                    green: g as f64 / 255.0,
                    blue: b as f64 / 255.0,
                    alpha: a as f64 / 255.0,
                })
            }
            _ => Err(ThemeError::InvalidHex(hex.to_string())),
        }
    }

    pub fn to_hex(&self) -> String {
        let r = (self.red * 255.0).round() as u8;
        let g = (self.green * 255.0).round() as u8;
        let b = (self.blue * 255.0).round() as u8;
        if (self.alpha - 1.0).abs() < 1e-4 {
            format!("#{:02x}{:02x}{:02x}", r, g, b)
        } else {
            let a = (self.alpha * 255.0).round() as u8;
            format!("#{:02x}{:02x}{:02x}{:02x}", a, r, g, b)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ColorSchemeJson", into = "ColorSchemeJson")]
pub struct ColorScheme {
    pub name: String,
    pub comment: Option<String>,
    pub foreground: RgbColor,
    pub background: RgbColor,
    pub cursor: Option<RgbColor>,
    pub cursor_foreground: Option<RgbColor>,
    pub palette: [RgbColor; 16],
}

#[derive(Serialize, Deserialize)]
struct ColorSchemeJson {
    name: String,
    #[serde(default)]
    comment: Option<String>,
    #[serde(rename = "foreground-color")]
    foreground_color: String,
    #[serde(rename = "background-color")]
    background_color: String,
    #[serde(default, rename = "cursor-background-color", alias = "cursor-color")]
    cursor_background_color: Option<String>,
    #[serde(default, rename = "cursor-foreground-color")]
    cursor_foreground_color: Option<String>,
    palette: Vec<String>,
}

impl TryFrom<ColorSchemeJson> for ColorScheme {
    type Error = ThemeError;

    fn try_from(raw: ColorSchemeJson) -> Result<Self, Self::Error> {
        if raw.palette.len() != 16 {
            return Err(ThemeError::InvalidPaletteLength(raw.palette.len()));
        }

        let foreground = RgbColor::from_hex(&raw.foreground_color)?;
        let background = RgbColor::from_hex(&raw.background_color)?;
        let cursor = raw
            .cursor_background_color
            .as_deref()
            .map(RgbColor::from_hex)
            .transpose()?;
        let cursor_foreground = raw
            .cursor_foreground_color
            .as_deref()
            .map(RgbColor::from_hex)
            .transpose()?;

        let mut pal = [
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
            RgbColor::rgb(0.0, 0.0, 0.0),
        ];

        for (i, hex_str) in raw.palette.iter().enumerate() {
            pal[i] = RgbColor::from_hex(hex_str)?;
        }

        Ok(ColorScheme {
            name: raw.name,
            comment: raw.comment,
            foreground,
            background,
            cursor,
            cursor_foreground,
            palette: pal,
        })
    }
}

impl From<ColorScheme> for ColorSchemeJson {
    fn from(scheme: ColorScheme) -> Self {
        ColorSchemeJson {
            name: scheme.name,
            comment: scheme.comment,
            foreground_color: scheme.foreground.to_hex(),
            background_color: scheme.background.to_hex(),
            cursor_background_color: scheme.cursor.as_ref().map(|c| c.to_hex()),
            cursor_foreground_color: scheme.cursor_foreground.as_ref().map(|c| c.to_hex()),
            palette: scheme.palette.iter().map(|c| c.to_hex()).collect(),
        }
    }
}

impl ColorScheme {
    pub fn from_json(json_str: &str) -> Result<Self, ThemeError> {
        let raw: ColorSchemeJson = serde_json::from_str(json_str)
            .map_err(|e| ThemeError::JsonError(e.to_string()))?;
        Self::try_from(raw)
    }

    pub fn to_json(&self) -> Result<String, ThemeError> {
        serde_json::to_string_pretty(self).map_err(|e| ThemeError::JsonError(e.to_string()))
    }

    pub fn tilix_dark() -> Self {
        make_scheme(
            "Tilix Dark",
            Some("Default Tilix dark color scheme"),
            "#c9cacc",
            "#1d1f21",
            Some("#ffffff"),
            Some("#1d1f21"),
            [
                "#1d1f21", "#cc342b", "#198844", "#fba922",
                "#3971ed", "#a36ac7", "#3971ed", "#c9cacc",
                "#282a2e", "#cc342b", "#198844", "#fba922",
                "#3971ed", "#a36ac7", "#3971ed", "#c9cacc",
            ],
        )
    }

    pub fn tilix_light() -> Self {
        make_scheme(
            "Tilix Light",
            Some("Default Tilix light color scheme"),
            "#2e3436",
            "#fafafa",
            Some("#2e3436"),
            Some("#fafafa"),
            [
                "#000000", "#cc0000", "#4e9a06", "#c4a000",
                "#3465a4", "#75507b", "#06989a", "#d3d7cf",
                "#555753", "#ef2929", "#8ae234", "#fce94f",
                "#729fcf", "#ad7fa8", "#34e2e2", "#eeeeec",
            ],
        )
    }

    pub fn solarized_dark() -> Self {
        make_scheme(
            "Solarized Dark",
            Some("Precision colors for machines and people"),
            "#839496",
            "#002b36",
            Some("#93a1a1"),
            Some("#002b36"),
            [
                "#073642", "#dc322f", "#859900", "#b58900",
                "#268bd2", "#d33682", "#2aa198", "#eee8d5",
                "#002b36", "#cb4b16", "#586e75", "#657b83",
                "#839496", "#6c71c4", "#93a1a1", "#fdf6e3",
            ],
        )
    }

    pub fn monokai() -> Self {
        make_scheme(
            "Monokai",
            Some("Monokai color scheme"),
            "#f8f8f2",
            "#272822",
            Some("#f8f8f0"),
            Some("#272822"),
            [
                "#272822", "#f92672", "#a6e22e", "#f4bf75",
                "#66d9ef", "#ae81ff", "#a1efe4", "#f8f8f2",
                "#75715e", "#f92672", "#a6e22e", "#f4bf75",
                "#66d9ef", "#ae81ff", "#a1efe4", "#f9f8f5",
            ],
        )
    }
}

fn make_scheme(
    name: &str,
    comment: Option<&str>,
    fg: &str,
    bg: &str,
    cursor: Option<&str>,
    cursor_fg: Option<&str>,
    palette: [&str; 16],
) -> ColorScheme {
    let mut pal = [
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
        RgbColor::rgb(0.0, 0.0, 0.0),
    ];
    for (i, hex) in palette.iter().enumerate() {
        pal[i] = RgbColor::from_hex(hex).unwrap();
    }
    ColorScheme {
        name: name.to_string(),
        comment: comment.map(|s| s.to_string()),
        foreground: RgbColor::from_hex(fg).unwrap(),
        background: RgbColor::from_hex(bg).unwrap(),
        cursor: cursor.map(|c| RgbColor::from_hex(c).unwrap()),
        cursor_foreground: cursor_fg.map(|c| RgbColor::from_hex(c).unwrap()),
        palette: pal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_color_from_hex_valid() {
        // 6-character hex with #
        let c1 = RgbColor::from_hex("#c9cacc").unwrap();
        assert!((c1.red - 201.0 / 255.0).abs() < 1e-4);
        assert!((c1.green - 202.0 / 255.0).abs() < 1e-4);
        assert!((c1.blue - 204.0 / 255.0).abs() < 1e-4);
        assert_eq!(c1.alpha, 1.0);
        assert_eq!(c1.to_hex(), "#c9cacc");

        // 6-character hex without #
        let c2 = RgbColor::from_hex("1d1f21").unwrap();
        assert_eq!(c2.to_hex(), "#1d1f21");

        // 8-character hex #AARRGGBB
        let c3 = RgbColor::from_hex("#80c9cacc").unwrap();
        assert!((c3.alpha - 128.0 / 255.0).abs() < 1e-4);
        assert!((c3.red - 201.0 / 255.0).abs() < 1e-4);
        assert_eq!(c3.to_hex(), "#80c9cacc");
    }

    #[test]
    fn test_rgb_color_from_hex_invalid() {
        assert!(matches!(
            RgbColor::from_hex("#invalid"),
            Err(ThemeError::InvalidHex(_))
        ));
        assert!(matches!(
            RgbColor::from_hex("#12345"),
            Err(ThemeError::InvalidHex(_))
        ));
        assert!(matches!(
            RgbColor::from_hex("#1234567"),
            Err(ThemeError::InvalidHex(_))
        ));
    }

    #[test]
    fn test_color_scheme_parse_tilix_json() {
        let json = r##"{
            "name": "Custom Test Theme",
            "comment": "A test theme",
            "foreground-color": "#c9cacc",
            "background-color": "#1d1f21",
            "cursor-background-color": "#ffffff",
            "cursor-foreground-color": "#000000",
            "palette": [
                "#000000", "#111111", "#222222", "#333333",
                "#444444", "#555555", "#666666", "#777777",
                "#888888", "#999999", "#aaaaaa", "#bbbbbb",
                "#cccccc", "#dddddd", "#eeeeee", "#ffffff"
            ]
        }"##;

        let scheme = ColorScheme::from_json(json).unwrap();
        assert_eq!(scheme.name, "Custom Test Theme");
        assert_eq!(scheme.comment.as_deref(), Some("A test theme"));
        assert_eq!(scheme.foreground.to_hex(), "#c9cacc");
        assert_eq!(scheme.background.to_hex(), "#1d1f21");
        assert_eq!(scheme.cursor.as_ref().unwrap().to_hex(), "#ffffff");
        assert_eq!(scheme.cursor_foreground.as_ref().unwrap().to_hex(), "#000000");
        assert_eq!(scheme.palette[0].to_hex(), "#000000");
        assert_eq!(scheme.palette[15].to_hex(), "#ffffff");

        // Round trip test
        let serialized = scheme.to_json().unwrap();
        let deserialized = ColorScheme::from_json(&serialized).unwrap();
        assert_eq!(scheme, deserialized);

        // Invalid palette length test
        let invalid_json = r##"{
            "name": "Bad Palette",
            "foreground-color": "#ffffff",
            "background-color": "#000000",
            "palette": ["#000000", "#ffffff"]
        }"##;
        assert!(matches!(
            ColorScheme::from_json(invalid_json),
            Err(ThemeError::InvalidPaletteLength(2))
        ));
    }

    #[test]
    fn test_builtin_color_schemes() {
        let dark = ColorScheme::tilix_dark();
        assert_eq!(dark.name, "Tilix Dark");
        assert_eq!(dark.palette.len(), 16);

        let light = ColorScheme::tilix_light();
        assert_eq!(light.name, "Tilix Light");
        assert_eq!(light.palette.len(), 16);

        let solarized = ColorScheme::solarized_dark();
        assert_eq!(solarized.name, "Solarized Dark");
        assert_eq!(solarized.palette.len(), 16);

        let monokai = ColorScheme::monokai();
        assert_eq!(monokai.name, "Monokai");
        assert_eq!(monokai.palette.len(), 16);
    }
}
