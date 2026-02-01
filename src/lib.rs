use core_foundation::string::CFString;
use core_foundation::attributed_string::CFMutableAttributedString;
use core_foundation::base::{TCFType, CFRange};
use core_text::font::CTFont;
use core_text::framesetter::CTFramesetter;
use core_text::line::CTLine;
use core_text::run::CTRun;
use core_graphics::path::CGPath;
use core_graphics::geometry::{CGRect, CGPoint, CGSize};
use std::ptr;
use std::os::raw::c_void;

// CTFontCreateUIFontForLanguage function signature
#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFontCreateUIFontForLanguage(
        ui_type: u32,
        size: f64,
        language: *const c_void,
    ) -> *mut c_void;
}

// kCTFontAttributeName constant declaration
// This is a CFStringRef constant exported by CoreText framework
#[link(name = "CoreText", kind = "framework")]
extern "C" {
    static kCTFontAttributeName: *const c_void;
}

// Helper function to safely get the font attribute name
// Returns None if the constant is not accessible
fn get_font_attribute_name() -> Option<*const c_void> {
    unsafe {
        // Check if the constant is accessible (not null)
        // Note: The constant should never be null if CoreText is properly linked
        if kCTFontAttributeName.is_null() {
            println!("DEBUG: Error - kCTFontAttributeName is null!");
            None
        } else {
            // Verify it's a valid pointer (not a very low address like 0xd)
            // Address 0xd (13) would indicate a corrupted or invalid pointer
            let addr = kCTFontAttributeName as usize;
            if addr < 0x1000 {
                println!("DEBUG: Error - kCTFontAttributeName has suspicious address: 0x{:x}", addr);
                None
            } else {
                Some(kCTFontAttributeName)
            }
        }
    }
}

const K_CTFONT_UIFONT_SYSTEM: u32 = 2;

fn create_base_font(size: f64) -> CTFont {
    unsafe {
        let font_ref = CTFontCreateUIFontForLanguage(
            K_CTFONT_UIFONT_SYSTEM,
            size,
            ptr::null(),
        );
        // Use the TCFType trait method
        <CTFont as TCFType>::wrap_under_create_rule(font_ref as *mut _)
    }
}

#[no_mangle]
pub extern "C" fn split_str_into_runs(text: *const i8, font_size: f64) {
    use std::ffi::CStr;
    
    let text_str = unsafe {
        CStr::from_ptr(text)
            .to_str()
            .unwrap_or("")
    };
    println!("DEBUG: Text: {}", text_str);
    println!("DEBUG: Font size: {}", font_size);
    split_str_into_runs_impl(text_str, font_size);
}

pub fn split_str_into_runs_impl(text: &str, font_size: f64) {
    // Create base font using system UI font
    let font = create_base_font(font_size);
    
    // Debug: Print the font name to see what we got
    unsafe {
        #[link(name = "CoreText", kind = "framework")]
        extern "C" {
            fn CTFontCopyPostScriptName(font: *const c_void) -> *const c_void;
        }
        let ps_name_ref = CTFontCopyPostScriptName(font.as_concrete_TypeRef() as *const c_void);
        if !ps_name_ref.is_null() {
            let ps_name_cf = CFString::wrap_under_create_rule(ps_name_ref as *mut _);
            println!("DEBUG: Created base font: {}", ps_name_cf.to_string());
        }
    }
    
    // Create CFString from Rust string
    let cf_string = CFString::new(text);
    
    // Create mutable attributed string
    let mut attributed_string = CFMutableAttributedString::new();
    attributed_string.replace_str(&cf_string, CFRange::init(0, 0));
    
    // Set the font attribute for the entire string using C API
    // This forces Core Text to use our font, though it may still create separate runs
    // for characters that need fallback fonts (Chinese, emoji, etc.)
    unsafe {
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFAttributedStringSetAttribute(
                aStr: *mut c_void,
                range: CFRange,
                attrName: *const c_void,
                value: *const c_void,
            );
            fn CFAttributedStringGetLength(aStr: *const c_void) -> isize;
        }
        
        // Get the actual length of the attributed string (in UTF-16 code units)
        let text_length = CFAttributedStringGetLength(attributed_string.as_concrete_TypeRef() as *const c_void);
        
        // Validate inputs before calling CFAttributedStringSetAttribute
        let attr_str_ptr = attributed_string.as_concrete_TypeRef() as *mut c_void;
        let font_ptr = font.as_concrete_TypeRef() as *const c_void;
        
        // Check that pointers are valid
        if attr_str_ptr.is_null() {
            println!("DEBUG: Error - attributed string pointer is null!");
            return;
        }
        if font_ptr.is_null() {
            println!("DEBUG: Error - font pointer is null!");
            return;
        }
        
        // Get the font attribute name key
        if let Some(font_key_ptr) = get_font_attribute_name() {
            println!("DEBUG: Font key pointer: {:p}", font_key_ptr);
            println!("DEBUG: Font pointer: {:p}", font_ptr);
            println!("DEBUG: Attributed string pointer: {:p}", attr_str_ptr);
            println!("DEBUG: Text length: {}", text_length);
            
            // Set the font attribute - the font must be retained, which TCFType handles
            // kCTFontAttributeName is already a CFStringRef, so we can use it directly
            CFAttributedStringSetAttribute(
                attr_str_ptr,
                CFRange::init(0, text_length as i64),
                font_key_ptr,
                font_ptr,
            );
            println!("DEBUG: Font attribute set on attributed string");
        } else {
            println!("DEBUG: Warning - Could not get font attribute name, skipping font setting");
        }
    }
    
    // Create framesetter
    let framesetter = CTFramesetter::new_with_attributed_string(attributed_string.as_concrete_TypeRef());
    
    // Create a path (rectangular path for layout)
    let bounds = CGRect::new(
        &CGPoint::new(0.0, 0.0),
        &CGSize::new(f64::MAX, f64::MAX),
    );
    let path = CGPath::from_rect(bounds, None);
    
    // Create frame
    let frame = framesetter.create_frame(
        CFRange::init(0, 0),
        &path,
    );
    
    // Get lines from frame using C API
    #[link(name = "CoreText", kind = "framework")]
    extern "C" {
        fn CTFrameGetLines(frame: *const c_void) -> *const c_void;
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
    }
    
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
    }
    
    unsafe {
        let lines_array = CTFrameGetLines(frame.as_concrete_TypeRef() as *const c_void);
        let line_count = CFArrayGetCount(lines_array) as usize;
        
        println!("Number of lines: {}", line_count);
        println!("Text: \"{}\"", text);
        println!("---");
        
        // Iterate through lines
        for line_idx in 0..line_count {
            let line_ref = CFArrayGetValueAtIndex(lines_array, line_idx as isize);
            let line = CTLine::wrap_under_get_rule(line_ref as *mut _);
            let runs = line.glyph_runs();
            
            println!("Line {}: {} runs", line_idx, runs.len());
            
            // Iterate through runs in each line
            for (run_idx, run) in runs.iter().enumerate() {
                let run = CTRun::wrap_under_get_rule(run.as_concrete_TypeRef());
                
                // Get font from run attributes using C API directly
                #[link(name = "CoreText", kind = "framework")]
                extern "C" {
                    fn CTRunGetAttributes(run: *const c_void) -> *const c_void;
                }
                
                let attributes_dict = CTRunGetAttributes(run.as_concrete_TypeRef() as *const c_void);
                
                let font_ptr = if !attributes_dict.is_null() {
                    // Get the font attribute name key
                    if let Some(font_key_ptr) = get_font_attribute_name() {
                        // Get the font value from the attributes dictionary
                        let font_value_ref = CFDictionaryGetValue(
                            attributes_dict,
                            font_key_ptr,
                        );
                        
                        if font_value_ref.is_null() {
                            ptr::null()
                        } else {
                            font_value_ref
                        }
                    } else {
                        ptr::null()
                    }
                } else {
                    ptr::null()
                };
                
                // Get PostScript name from font
                let postscript_name = if !font_ptr.is_null() {
                    #[link(name = "CoreText", kind = "framework")]
                    extern "C" {
                        fn CTFontCopyPostScriptName(font: *const c_void) -> *const c_void;
                    }
                    
                    let ps_name_ref = CTFontCopyPostScriptName(font_ptr);
                    if ps_name_ref.is_null() {
                        String::from("(null)")
                    } else {
                        let ps_name_cf = CFString::wrap_under_create_rule(ps_name_ref as *mut _);
                        ps_name_cf.to_string()
                    }
                } else {
                    String::from("(no font)")
                };
                
                // Get run text range using C API
                // Note: CFRange uses UTF-16 code units, need to convert to UTF-8 byte indices
                #[link(name = "CoreText", kind = "framework")]
                extern "C" {
                    fn CTRunGetStringRange(run: *const c_void) -> CFRange;
                }
                let range = CTRunGetStringRange(run.as_concrete_TypeRef() as *const c_void);
                let start_utf16 = range.location as usize;
                let length_utf16 = range.length as usize;
                
                // Convert UTF-16 indices to UTF-8 byte indices
                let text_utf16: Vec<u16> = text.encode_utf16().collect();
                let run_text = if start_utf16 + length_utf16 <= text_utf16.len() {
                    let utf16_slice = &text_utf16[start_utf16..start_utf16 + length_utf16];
                    match String::from_utf16(utf16_slice) {
                        Ok(s) => s,
                        Err(_) => String::from(""),
                    }
                } else {
                    String::from("")
                };
                
                println!(
                    "  Run {}: \"{}\" | Font pointer: {:p} | PostScript name: {}",
                    run_idx, run_text, font_ptr, postscript_name
                );
            }
            println!("---");
        }
    }
}
