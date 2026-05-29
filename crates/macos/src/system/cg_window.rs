use core_foundation::{base::CFType, dictionary::CFDictionary, string::CFString};

type WindowDictionary = CFDictionary<CFString, CFType>;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WindowRecord {
    pub(crate) app_name: String,
    pub(crate) pid: i32,
    pub(crate) title: Option<String>,
    pub(crate) window_number: i64,
    pub(crate) area: f64,
}

pub(crate) fn visible_window_records() -> Vec<WindowRecord> {
    window_dictionaries()
        .into_iter()
        .filter_map(|dict| {
            if int_field(&dict, "kCGWindowLayer")? != 0 {
                return None;
            }

            let pid = int_field(&dict, "kCGWindowOwnerPID")? as i32;
            if pid <= 0 {
                return None;
            }

            let app_name = string_field(&dict, "kCGWindowOwnerName")?;
            if app_name.is_empty() {
                return None;
            }

            Some(WindowRecord {
                app_name,
                pid,
                title: string_field(&dict, "kCGWindowName").filter(|title| !title.is_empty()),
                window_number: int_field(&dict, "kCGWindowNumber").unwrap_or(0),
                area: area_field(&dict, "kCGWindowBounds").unwrap_or(0.0),
            })
        })
        .collect()
}

fn window_dictionaries() -> Vec<WindowDictionary> {
    use crate::cf_type::borrowed_cf_dictionary;
    use core_graphics::display::CGDisplay;
    use core_graphics::window::{
        kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
    };

    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let Some(array) = CGDisplay::window_list_info(options, None) else {
        return Vec::new();
    };

    array
        .get_all_values()
        .into_iter()
        .filter_map(|raw| borrowed_cf_dictionary(raw as core_foundation::base::CFTypeRef))
        .collect()
}

fn int_field(dict: &WindowDictionary, key: &str) -> Option<i64> {
    use crate::cf_type::borrowed_cf_number;
    use core_foundation::base::TCFType;

    let key = CFString::new(key);
    dict.find(&key)
        .and_then(|value| borrowed_cf_number(value.as_concrete_TypeRef()))
        .and_then(|number| number.to_i64())
}

fn string_field(dict: &WindowDictionary, key: &str) -> Option<String> {
    use crate::cf_type::borrowed_cf_string;
    use core_foundation::base::TCFType;

    let key = CFString::new(key);
    dict.find(&key)
        .and_then(|value| borrowed_cf_string(value.as_concrete_TypeRef()))
        .map(|value| value.to_string())
}

fn area_field(dict: &WindowDictionary, key: &str) -> Option<f64> {
    use crate::cf_type::borrowed_cf_dictionary;
    use core_foundation::base::TCFType;

    let bounds = dict
        .find(CFString::new(key))
        .and_then(|value| borrowed_cf_dictionary(value.as_concrete_TypeRef()))?;
    let width = int_or_float_field(&bounds, "Width").unwrap_or(0.0);
    let height = int_or_float_field(&bounds, "Height").unwrap_or(0.0);
    Some(width * height)
}

fn int_or_float_field(dict: &WindowDictionary, key: &str) -> Option<f64> {
    use crate::cf_type::borrowed_cf_number;
    use core_foundation::base::TCFType;

    let key = CFString::new(key);
    dict.find(&key)
        .and_then(|value| borrowed_cf_number(value.as_concrete_TypeRef()))
        .and_then(|number| number.to_f64())
}
