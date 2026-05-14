#[cfg(target_os = "macos")]
mod imp {
    use crate::tree::AXElement;
    use accessibility_sys::{
        AXUIElementCopyActionNames, AXUIElementIsAttributeSettable, kAXErrorSuccess,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFEqual, CFType, CFTypeRef, TCFType},
        string::CFString,
    };
    use std::os::raw::c_uchar;

    pub fn is_attr_settable(el: &AXElement, attr: &str) -> bool {
        let cf_attr = CFString::new(attr);
        let mut settable: c_uchar = 0;
        let err = unsafe {
            AXUIElementIsAttributeSettable(el.0, cf_attr.as_concrete_TypeRef(), &mut settable)
        };
        err == kAXErrorSuccess && settable != 0
    }

    pub fn copy_action_names(el: &AXElement) -> Vec<String> {
        let mut actions_ref: core_foundation_sys::array::CFArrayRef = std::ptr::null();
        let err = unsafe { AXUIElementCopyActionNames(el.0, &mut actions_ref) };
        if err != kAXErrorSuccess || actions_ref.is_null() {
            return Vec::new();
        }

        let actions: CFArray<CFType> = unsafe { TCFType::wrap_under_create_rule(actions_ref) };
        let mut result = Vec::with_capacity(actions.len() as usize);
        for i in 0..actions.len() {
            if let Some(name) = actions.get(i).and_then(|v| v.downcast::<CFString>()) {
                result.push(name.to_string());
            }
        }
        result
    }

    pub fn same_element(a: &AXElement, b: &AXElement) -> bool {
        unsafe { CFEqual(a.0 as CFTypeRef, b.0 as CFTypeRef) != 0 }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::AXElement;

    pub fn is_attr_settable(_el: &AXElement, _attr: &str) -> bool {
        false
    }

    pub fn copy_action_names(_el: &AXElement) -> Vec<String> {
        Vec::new()
    }

    pub fn same_element(_a: &AXElement, _b: &AXElement) -> bool {
        false
    }
}

pub use imp::{copy_action_names, is_attr_settable, same_element};
