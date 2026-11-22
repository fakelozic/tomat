//! Whether KDE is dark or light, read from its colour file.

use std::io::Read;
use std::path::Path;

use super::Look;

/// No real colour file comes near this. It bounds what a bad one can cost.
const LIMIT: u64 = 1024 * 1024;

/// The look for the desktop's current colours. KDE's own default is light.
#[must_use]
pub fn current(config: &Path) -> Look {
    ["kdeglobals", "kdedefaults/kdeglobals"]
        .iter()
        .filter_map(|name| read(&config.join(name)))
        .find_map(|text| look_of(&text))
        .unwrap_or(Look::Light)
}

fn read(path: &Path) -> Option<String> {
    // A pipe or a device here could block forever.
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok()?.take(LIMIT).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The look for the window background in a `kdeglobals` text.
fn look_of(text: &str) -> Option<Look> {
    let mut in_window = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_window = line == "[Colors:Window]";
        } else if in_window && let Some(value) = line.strip_prefix("BackgroundNormal=") {
            let [r, g, b] = rgb(value)?.map(f32::from);
            let brightness = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            return Some(if brightness > 127.5 { Look::Light } else { Look::Smoked });
        }
    }
    None
}

fn rgb(value: &str) -> Option<[u8; 3]> {
    if let Some(hex) = value.strip_prefix('#') {
        // KDE also writes #AARRGGBB.
        let hex = if hex.len() == 8 { hex.get(2..)? } else { hex };
        let [0, r, g, b] = u32::from_str_radix(hex, 16).ok()?.to_be_bytes() else {
            return None;
        };
        return (hex.len() == 6).then_some([r, g, b]);
    }
    let mut parts = value.split(',').map(|part| part.trim().parse().ok());
    Some([parts.next()??, parts.next()??, parts.next()??])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_breeze_dark_and_light() {
        let file = |value: &str| format!("[Colors:Window]\nBackgroundNormal={value}\n");
        assert_eq!(look_of(&file("32,35,38")), Some(Look::Smoked));
        assert_eq!(look_of(&file("239,240,241")), Some(Look::Light));
        assert_eq!(look_of(&file("239,240,241,255")), Some(Look::Light));
        assert_eq!(look_of(&file("#eff0f1")), Some(Look::Light));
        assert_eq!(look_of(&file("#ff17191a")), Some(Look::Smoked));
    }

    #[test]
    fn only_the_window_group_counts() {
        let text = "[Colors:View]\nBackgroundNormal=255,255,255\n\
                    [Colors:Window][Inactive]\nBackgroundNormal=250,250,250\n\
                    [Colors:Window]\nBackgroundNormal=20,20,20\n";
        assert_eq!(look_of(text), Some(Look::Smoked));
    }

    #[test]
    fn garbage_is_no_answer() {
        for value in ["", "red", "300,1,1", "#12", "#zzzzzz", "1,2"] {
            let text = format!("[Colors:Window]\nBackgroundNormal={value}\n");
            assert_eq!(look_of(&text), None, "{value}");
        }
    }
}
