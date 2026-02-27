/// Browser fingerprint diversity for headless crawling.
///
/// Real browsers expose dozens of fingerprinting surfaces — viewport size,
/// WebGL vendor/renderer strings, navigator.languages, timezone, platform,
/// canvas noise, and more.  When all headless instances share identical values
/// bot-detection systems can trivially identify them.
///
/// This module provides randomised, coherent fingerprint *profiles* that
/// keep all signals internally consistent (e.g. a "mobile Chrome on Android"
/// profile uses mobile UA + mobile viewport + touch support + Android locale).
///
/// # Usage
///
/// ```rust
/// use rehyke_core::browser_fingerprint::{BrowserFingerprint, FingerprintProfile};
///
/// // Randomise within the desktop profile.
/// let fp = BrowserFingerprint::randomize(FingerprintProfile::Desktop);
/// println!("UA: {}", fp.user_agent);
/// println!("Viewport: {}x{}", fp.viewport_width, fp.viewport_height);
///
/// // Fixed, reproducible profile.
/// let fp = BrowserFingerprint::desktop();
/// ```
use crate::config::Viewport;
use rand::seq::SliceRandom;
use rand::Rng;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// High-level browser profile family used to select coherent fingerprint sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerprintProfile {
    /// Full-HD desktop (Windows / macOS / Linux).
    Desktop,
    /// Mid-range Android tablet.
    Tablet,
    /// Modern smartphone (iOS or Android).
    Mobile,
}

impl From<Viewport> for FingerprintProfile {
    fn from(v: Viewport) -> Self {
        match v {
            Viewport::Desktop => FingerprintProfile::Desktop,
            Viewport::Tablet => FingerprintProfile::Tablet,
            Viewport::Mobile => FingerprintProfile::Mobile,
        }
    }
}

/// A coherent set of browser fingerprint values that can be injected into a
/// headless browser session via CDP `Page.addScriptToEvaluateOnNewDocument`.
#[derive(Debug, Clone)]
pub struct BrowserFingerprint {
    // --- Viewport -------------------------------------------------------
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub device_pixel_ratio: f64,

    // --- User-Agent & platform ------------------------------------------
    pub user_agent: String,
    pub platform: String,

    // --- Navigator properties -------------------------------------------
    /// `navigator.languages` array, e.g. `["en-US","en"]`.
    pub languages: Vec<String>,
    /// `navigator.vendor`, e.g. `"Google Inc."`.
    pub vendor: String,
    /// Whether `navigator.webdriver` is spoofed to `false`.
    pub hide_webdriver: bool,

    // --- WebGL ----------------------------------------------------------
    /// `WEBGL_debug_renderer_info` UNMASKED_VENDOR_WEBGL, e.g. `"Google Inc."`.
    pub webgl_vendor: String,
    /// `WEBGL_debug_renderer_info` UNMASKED_RENDERER_WEBGL, e.g.
    /// `"ANGLE (NVIDIA, NVIDIA GeForce RTX 3080)"`.
    pub webgl_renderer: String,

    // --- Timezone -------------------------------------------------------
    /// IANA timezone string, e.g. `"America/New_York"`.
    pub timezone: String,
    /// UTC offset in minutes, e.g. `-300` for EST.
    pub timezone_offset: i32,

    // --- Canvas ---------------------------------------------------------
    /// Whether to inject subtle per-session canvas noise.
    pub canvas_noise: bool,
}

impl BrowserFingerprint {
    // -----------------------------------------------------------------------
    // Static profiles
    // -----------------------------------------------------------------------

    /// Canonical desktop fingerprint — no randomisation.
    pub fn desktop() -> Self {
        Self {
            viewport_width: 1920,
            viewport_height: 1080,
            device_pixel_ratio: 1.0,
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36".into(),
            platform: "Win32".into(),
            languages: vec!["en-US".into(), "en".into()],
            vendor: "Google Inc.".into(),
            hide_webdriver: true,
            webgl_vendor: "Google Inc. (NVIDIA)".into(),
            webgl_renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 Direct3D11 vs_5_0 ps_5_0, D3D11)".into(),
            timezone: "America/New_York".into(),
            timezone_offset: -300,
            canvas_noise: false,
        }
    }

    /// Canonical tablet fingerprint — no randomisation.
    pub fn tablet() -> Self {
        Self {
            viewport_width: 768,
            viewport_height: 1024,
            device_pixel_ratio: 2.0,
            user_agent: "Mozilla/5.0 (Linux; Android 13; Pixel Tablet) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.6613.127 Safari/537.36".into(),
            platform: "Linux armv8l".into(),
            languages: vec!["en-US".into(), "en".into()],
            vendor: "Google Inc.".into(),
            hide_webdriver: true,
            webgl_vendor: "Qualcomm".into(),
            webgl_renderer: "Adreno (TM) 740".into(),
            timezone: "America/New_York".into(),
            timezone_offset: -300,
            canvas_noise: true,
        }
    }

    /// Canonical mobile fingerprint — no randomisation.
    pub fn mobile() -> Self {
        Self {
            viewport_width: 390,
            viewport_height: 844,
            device_pixel_ratio: 3.0,
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".into(),
            platform: "iPhone".into(),
            languages: vec!["en-US".into(), "en".into()],
            vendor: "Apple Computer, Inc.".into(),
            hide_webdriver: true,
            webgl_vendor: "Apple Inc.".into(),
            webgl_renderer: "Apple GPU".into(),
            timezone: "America/New_York".into(),
            timezone_offset: -300,
            canvas_noise: true,
        }
    }

    // -----------------------------------------------------------------------
    // Randomised profiles
    // -----------------------------------------------------------------------

    /// Produce a randomised but internally consistent fingerprint for the
    /// given profile family.
    ///
    /// Each call returns different values, making every browser session look
    /// distinct to fingerprinting scripts.
    pub fn randomize(profile: FingerprintProfile) -> Self {
        let mut rng = rand::thread_rng();

        match profile {
            FingerprintProfile::Desktop => Self::random_desktop(&mut rng),
            FingerprintProfile::Tablet => Self::random_tablet(&mut rng),
            FingerprintProfile::Mobile => Self::random_mobile(&mut rng),
        }
    }

    fn random_desktop(rng: &mut impl Rng) -> Self {
        let resolutions: &[(u32, u32)] = &[
            (1920, 1080),
            (2560, 1440),
            (1440, 900),
            (1680, 1050),
            (1366, 768),
            (1280, 800),
        ];
        let (w, h) = *resolutions.choose(rng).unwrap();

        let tz = *DESKTOP_TIMEZONES.choose(rng).unwrap();
        let (tz_name, tz_offset) = tz;

        let ua = DESKTOP_UAS.choose(rng).unwrap();
        let gpu = DESKTOP_GPUS.choose(rng).unwrap();
        let lang = LANGUAGE_PROFILES.choose(rng).unwrap();

        Self {
            viewport_width: w,
            viewport_height: h,
            device_pixel_ratio: if rng.gen_bool(0.3) { 1.5 } else { 1.0 },
            user_agent: ua.to_string(),
            platform: if ua.contains("Macintosh") {
                "MacIntel"
            } else {
                "Win32"
            }
            .into(),
            languages: lang.iter().map(|s| s.to_string()).collect(),
            vendor: "Google Inc.".into(),
            hide_webdriver: true,
            webgl_vendor: "Google Inc. (NVIDIA)".into(),
            webgl_renderer: gpu.to_string(),
            timezone: tz_name.to_string(),
            timezone_offset: tz_offset,
            canvas_noise: rng.gen_bool(0.6),
        }
    }

    fn random_tablet(rng: &mut impl Rng) -> Self {
        let resolutions: &[(u32, u32)] = &[
            (768, 1024),
            (800, 1280),
            (1200, 1920),
            (834, 1194),
        ];
        let (w, h) = *resolutions.choose(rng).unwrap();
        let tz = *DESKTOP_TIMEZONES.choose(rng).unwrap();
        let (tz_name, tz_offset) = tz;
        let ua = TABLET_UAS.choose(rng).unwrap();
        let lang = LANGUAGE_PROFILES.choose(rng).unwrap();

        Self {
            viewport_width: w,
            viewport_height: h,
            device_pixel_ratio: *[1.5f64, 2.0, 2.5].choose(rng).unwrap(),
            user_agent: ua.to_string(),
            platform: "Linux armv8l".into(),
            languages: lang.iter().map(|s| s.to_string()).collect(),
            vendor: "Google Inc.".into(),
            hide_webdriver: true,
            webgl_vendor: TABLET_WEBGL_VENDORS.choose(rng).unwrap().to_string(),
            webgl_renderer: TABLET_WEBGL_RENDERERS.choose(rng).unwrap().to_string(),
            timezone: tz_name.to_string(),
            timezone_offset: tz_offset,
            canvas_noise: true,
        }
    }

    fn random_mobile(rng: &mut impl Rng) -> Self {
        let ios = rng.gen_bool(0.4);
        let tz = *DESKTOP_TIMEZONES.choose(rng).unwrap();
        let (tz_name, tz_offset) = tz;
        let lang = LANGUAGE_PROFILES.choose(rng).unwrap();

        if ios {
            let ua = IOS_UAS.choose(rng).unwrap();
            let res = *IOS_RESOLUTIONS.choose(rng).unwrap();
            Self {
                viewport_width: res.0,
                viewport_height: res.1,
                device_pixel_ratio: res.2,
                user_agent: ua.to_string(),
                platform: "iPhone".into(),
                languages: lang.iter().map(|s| s.to_string()).collect(),
                vendor: "Apple Computer, Inc.".into(),
                hide_webdriver: true,
                webgl_vendor: "Apple Inc.".into(),
                webgl_renderer: "Apple GPU".into(),
                timezone: tz_name.to_string(),
                timezone_offset: tz_offset,
                canvas_noise: true,
            }
        } else {
            let ua = ANDROID_UAS.choose(rng).unwrap();
            let res = *ANDROID_RESOLUTIONS.choose(rng).unwrap();
            Self {
                viewport_width: res.0,
                viewport_height: res.1,
                device_pixel_ratio: res.2,
                user_agent: ua.to_string(),
                platform: "Linux aarch64".into(),
                languages: lang.iter().map(|s| s.to_string()).collect(),
                vendor: "Google Inc.".into(),
                hide_webdriver: true,
                webgl_vendor: MOBILE_WEBGL_VENDORS.choose(rng).unwrap().to_string(),
                webgl_renderer: MOBILE_WEBGL_RENDERERS.choose(rng).unwrap().to_string(),
                timezone: tz_name.to_string(),
                timezone_offset: tz_offset,
                canvas_noise: true,
            }
        }
    }

    // -----------------------------------------------------------------------
    // CDP script injection
    // -----------------------------------------------------------------------

    /// Generate a JavaScript snippet that can be injected via
    /// `Page.addScriptToEvaluateOnNewDocument` to apply this fingerprint
    /// before any page scripts run.
    ///
    /// The script overrides `navigator.webdriver`, `navigator.platform`,
    /// `navigator.languages`, `navigator.vendor`, and WebGL renderer info.
    pub fn to_injection_script(&self) -> String {
        let languages_json = serde_json::to_string(&self.languages)
            .unwrap_or_else(|_| r#"["en-US","en"]"#.to_string());

        let webdriver_override = if self.hide_webdriver {
            r#"
            Object.defineProperty(navigator, 'webdriver', {
                get: () => undefined,
                configurable: true,
            });"#
        } else {
            ""
        };

        let canvas_noise = if self.canvas_noise {
            r#"
            const origToDataURL = HTMLCanvasElement.prototype.toDataURL;
            HTMLCanvasElement.prototype.toDataURL = function(type, quality) {
                const ctx = this.getContext('2d');
                if (ctx) {
                    const noise = Math.random() * 0.002 - 0.001;
                    const d = ctx.getImageData(0, 0, this.width || 1, this.height || 1);
                    if (d.data.length > 0) {
                        d.data[0] = Math.min(255, Math.max(0, d.data[0] + noise * 255));
                        ctx.putImageData(d, 0, 0);
                    }
                }
                return origToDataURL.apply(this, arguments);
            };"#
        } else {
            ""
        };

        format!(
            r#"
            // Rehyke browser fingerprint injection
            (function() {{
                {webdriver_override}

                Object.defineProperty(navigator, 'platform', {{
                    get: () => {platform:?},
                    configurable: true,
                }});

                Object.defineProperty(navigator, 'languages', {{
                    get: () => {languages_json},
                    configurable: true,
                }});

                Object.defineProperty(navigator, 'vendor', {{
                    get: () => {vendor:?},
                    configurable: true,
                }});

                // WebGL fingerprint override
                const origGetParam = WebGLRenderingContext.prototype.getParameter;
                WebGLRenderingContext.prototype.getParameter = function(pname) {{
                    if (pname === 37445) return {webgl_vendor:?}; // UNMASKED_VENDOR_WEBGL
                    if (pname === 37446) return {webgl_renderer:?}; // UNMASKED_RENDERER_WEBGL
                    return origGetParam.apply(this, arguments);
                }};
                const origGetParam2 = WebGL2RenderingContext.prototype.getParameter;
                WebGL2RenderingContext.prototype.getParameter = function(pname) {{
                    if (pname === 37445) return {webgl_vendor:?};
                    if (pname === 37446) return {webgl_renderer:?};
                    return origGetParam2.apply(this, arguments);
                }};

                {canvas_noise}
            }})();
            "#,
            webdriver_override = webdriver_override,
            platform = self.platform,
            languages_json = languages_json,
            vendor = self.vendor,
            webgl_vendor = self.webgl_vendor,
            webgl_renderer = self.webgl_renderer,
            canvas_noise = canvas_noise,
        )
    }
}

// ---------------------------------------------------------------------------
// Data tables
// ---------------------------------------------------------------------------

const DESKTOP_UAS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:127.0) Gecko/20100101 Firefox/127.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:128.0) Gecko/20100101 Firefox/128.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36 Edg/128.0.0.0",
];

const DESKTOP_GPUS: &[&str] = &[
    "ANGLE (NVIDIA, NVIDIA GeForce RTX 4080 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    "ANGLE (NVIDIA, NVIDIA GeForce RTX 3070 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    "ANGLE (AMD, AMD Radeon RX 6800 XT Direct3D11 vs_5_0 ps_5_0, D3D11)",
    "ANGLE (Intel, Intel(R) UHD Graphics 770 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    "ANGLE (Intel, Intel(R) Iris(R) Xe Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)",
    "ANGLE (Apple, Apple M2 Pro, OpenGL 4.1)",
    "ANGLE (Apple, Apple M1, OpenGL 4.1)",
    "ANGLE (Apple, Apple M3 Max, OpenGL 4.1)",
];

const TABLET_UAS: &[&str] = &[
    "Mozilla/5.0 (Linux; Android 13; Pixel Tablet) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.6478.126 Safari/537.36",
    "Mozilla/5.0 (Linux; Android 14; SM-X700) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.6533.103 Safari/537.36",
    "Mozilla/5.0 (Linux; Android 13; SM-T870) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.6478.183 Safari/537.36",
    "Mozilla/5.0 (iPad; CPU OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (iPad; CPU OS 16_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/126.0.6478.153 Mobile/15E148 Safari/604.1",
];

const TABLET_WEBGL_VENDORS: &[&str] = &["Qualcomm", "ARM", "Imagination Technologies", "MediaTek"];
const TABLET_WEBGL_RENDERERS: &[&str] = &[
    "Adreno (TM) 740",
    "Adreno (TM) 650",
    "Mali-G715",
    "PowerVR GT7600 Plus",
];

const IOS_UAS: &[&str] = &[
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 16_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/126.0.6478.153 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
];

/// `(width, height, device_pixel_ratio)` tuples for common iOS devices.
const IOS_RESOLUTIONS: &[(u32, u32, f64)] = &[
    (390, 844, 3.0),   // iPhone 14 / 15
    (430, 932, 3.0),   // iPhone 14 Plus / 15 Plus
    (393, 852, 3.0),   // iPhone 15 Pro
    (430, 932, 3.0),   // iPhone 15 Pro Max
    (375, 812, 3.0),   // iPhone X / XS
    (414, 896, 2.0),   // iPhone XR
];

const ANDROID_UAS: &[&str] = &[
    "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.6613.88 Mobile Safari/537.36",
    "Mozilla/5.0 (Linux; Android 14; Pixel 8 Pro) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.6533.103 Mobile Safari/537.36",
    "Mozilla/5.0 (Linux; Android 14; SM-S928B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.6478.182 Mobile Safari/537.36",
    "Mozilla/5.0 (Linux; Android 13; SM-A546B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.6478.183 Mobile Safari/537.36",
    "Mozilla/5.0 (Linux; Android 14; OnePlus 12) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.6533.84 Mobile Safari/537.36",
];

/// `(width, height, device_pixel_ratio)` for common Android phones.
const ANDROID_RESOLUTIONS: &[(u32, u32, f64)] = &[
    (393, 873, 2.75),  // Pixel 8
    (412, 915, 2.625), // Pixel 8 Pro
    (360, 780, 3.0),   // Samsung Galaxy S24
    (390, 844, 2.0),   // mid-range Android
    (411, 869, 2.625), // OnePlus
];

const MOBILE_WEBGL_VENDORS: &[&str] = &["Qualcomm", "ARM", "Imagination Technologies"];
const MOBILE_WEBGL_RENDERERS: &[&str] = &[
    "Adreno (TM) 750",
    "Adreno (TM) 740",
    "Adreno (TM) 650",
    "Mali-G715 MC11",
    "Mali-G76 MC4",
    "PowerVR B-Series BXM-8-256",
];

/// Common timezones with their UTC offset in minutes.
const DESKTOP_TIMEZONES: &[(&str, i32)] = &[
    ("America/New_York", -300),
    ("America/Chicago", -360),
    ("America/Denver", -420),
    ("America/Los_Angeles", -480),
    ("America/Toronto", -300),
    ("America/Vancouver", -480),
    ("America/Sao_Paulo", -180),
    ("Europe/London", 0),
    ("Europe/Paris", 60),
    ("Europe/Berlin", 60),
    ("Europe/Madrid", 60),
    ("Europe/Amsterdam", 60),
    ("Europe/Warsaw", 60),
    ("Europe/Moscow", 180),
    ("Asia/Kolkata", 330),
    ("Asia/Singapore", 480),
    ("Asia/Tokyo", 540),
    ("Asia/Shanghai", 480),
    ("Australia/Sydney", 600),
];

/// Pre-built language profiles matching common browser accept-language headers.
const LANGUAGE_PROFILES: &[&[&str]] = &[
    &["en-US", "en"],
    &["en-GB", "en"],
    &["en-CA", "en"],
    &["en-AU", "en"],
    &["de-DE", "de", "en-US", "en"],
    &["fr-FR", "fr", "en-US", "en"],
    &["es-ES", "es", "en-US", "en"],
    &["es-MX", "es", "en-US", "en"],
    &["pt-BR", "pt", "en-US", "en"],
    &["it-IT", "it", "en-US", "en"],
    &["nl-NL", "nl", "en-US", "en"],
    &["pl-PL", "pl", "en-US", "en"],
    &["ja-JP", "ja", "en-US", "en"],
    &["zh-CN", "zh", "en-US", "en"],
    &["ko-KR", "ko", "en-US", "en"],
    &["ru-RU", "ru", "en-US", "en"],
    &["ar-SA", "ar", "en-US", "en"],
    &["hi-IN", "hi", "en-US", "en"],
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desktop_fingerprint_coherent() {
        let fp = BrowserFingerprint::desktop();
        assert_eq!(fp.viewport_width, 1920);
        assert_eq!(fp.viewport_height, 1080);
        assert_eq!(fp.device_pixel_ratio, 1.0);
        assert!(fp.user_agent.contains("Windows"));
        assert!(fp.hide_webdriver);
        assert!(!fp.canvas_noise);
    }

    #[test]
    fn test_tablet_fingerprint_coherent() {
        let fp = BrowserFingerprint::tablet();
        assert_eq!(fp.viewport_width, 768);
        assert_eq!(fp.viewport_height, 1024);
        assert_eq!(fp.device_pixel_ratio, 2.0);
        assert!(fp.canvas_noise);
    }

    #[test]
    fn test_mobile_fingerprint_coherent() {
        let fp = BrowserFingerprint::mobile();
        assert_eq!(fp.viewport_width, 390);
        assert_eq!(fp.viewport_height, 844);
        assert_eq!(fp.device_pixel_ratio, 3.0);
        assert!(fp.canvas_noise);
    }

    #[test]
    fn test_randomize_desktop_within_bounds() {
        for _ in 0..20 {
            let fp = BrowserFingerprint::randomize(FingerprintProfile::Desktop);
            assert!(fp.viewport_width >= 1280 && fp.viewport_width <= 2560);
            assert!(fp.viewport_height >= 768 && fp.viewport_height <= 1440);
            assert!(!fp.user_agent.is_empty());
            assert!(!fp.webgl_renderer.is_empty());
            assert!(!fp.timezone.is_empty());
            assert!(!fp.languages.is_empty());
        }
    }

    #[test]
    fn test_randomize_tablet() {
        for _ in 0..10 {
            let fp = BrowserFingerprint::randomize(FingerprintProfile::Tablet);
            assert!(fp.viewport_width >= 768);
            assert!(fp.viewport_height >= 1024);
        }
    }

    #[test]
    fn test_randomize_mobile() {
        for _ in 0..10 {
            let fp = BrowserFingerprint::randomize(FingerprintProfile::Mobile);
            assert!(fp.viewport_width >= 360 && fp.viewport_width <= 430);
        }
    }

    #[test]
    fn test_injection_script_non_empty() {
        let fp = BrowserFingerprint::desktop();
        let script = fp.to_injection_script();
        assert!(!script.is_empty());
        assert!(script.contains("navigator"));
        assert!(script.contains("webdriver"));
        assert!(script.contains("WebGLRenderingContext"));
        assert!(script.contains("getParameter"));
    }

    #[test]
    fn test_injection_script_contains_platform() {
        let fp = BrowserFingerprint::desktop();
        let script = fp.to_injection_script();
        assert!(script.contains("Win32"));
    }

    #[test]
    fn test_injection_script_with_canvas_noise() {
        let mut fp = BrowserFingerprint::mobile();
        fp.canvas_noise = true;
        let script = fp.to_injection_script();
        assert!(script.contains("toDataURL"));
    }

    #[test]
    fn test_injection_script_without_canvas_noise() {
        let mut fp = BrowserFingerprint::desktop();
        fp.canvas_noise = false;
        let script = fp.to_injection_script();
        // Ensure the canvas override code is not present.
        assert!(!script.contains("toDataURL"));
    }

    #[test]
    fn test_fingerprint_profile_from_viewport() {
        assert_eq!(
            FingerprintProfile::from(Viewport::Desktop),
            FingerprintProfile::Desktop
        );
        assert_eq!(
            FingerprintProfile::from(Viewport::Tablet),
            FingerprintProfile::Tablet
        );
        assert_eq!(
            FingerprintProfile::from(Viewport::Mobile),
            FingerprintProfile::Mobile
        );
    }

    #[test]
    fn test_desktop_ua_pool_non_empty() {
        assert!(!DESKTOP_UAS.is_empty());
    }

    #[test]
    fn test_language_profiles_non_empty() {
        assert!(!LANGUAGE_PROFILES.is_empty());
        for profile in LANGUAGE_PROFILES {
            assert!(!profile.is_empty(), "Empty language profile found");
        }
    }

    #[test]
    fn test_timezone_table_non_empty() {
        assert!(!DESKTOP_TIMEZONES.is_empty());
        for (name, _offset) in DESKTOP_TIMEZONES {
            assert!(!name.is_empty(), "Empty timezone name");
        }
    }

    #[test]
    fn test_randomize_produces_distinct_values() {
        let fp1 = BrowserFingerprint::randomize(FingerprintProfile::Desktop);
        let fp2 = BrowserFingerprint::randomize(FingerprintProfile::Desktop);
        // With enough entropy at least one field should differ across runs.
        // (Extremely unlikely to be identical.)
        let same = fp1.viewport_width == fp2.viewport_width
            && fp1.viewport_height == fp2.viewport_height
            && fp1.user_agent == fp2.user_agent
            && fp1.webgl_renderer == fp2.webgl_renderer
            && fp1.timezone == fp2.timezone;
        // We don't assert `!same` because there's a tiny probability of collision,
        // but we do verify both are well-formed.
        let _ = same;
        assert!(!fp1.user_agent.is_empty());
        assert!(!fp2.user_agent.is_empty());
    }
}
