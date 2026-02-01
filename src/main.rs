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

// kCTFontAttributeName constant
#[link(name = "CoreText", kind = "framework")]
extern "C" {
    static kCTFontAttributeName: *const c_void;
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
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFAttributedStringSetAttribute(
        astring: *mut c_void,
        range: CFRange,
        attrName: *const c_void,
        value: *const c_void,
    );
}

fn split_str_into_runs(text: &str, font_size: f64) {
    // Create base font (kept for reference, though not explicitly set on attributed string)
    let _font = create_base_font(font_size);
    
    // Create CFString from Rust string
    let cf_string = CFString::new(text);
    
    // Create mutable attributed string
    let mut attributed_string = CFMutableAttributedString::new();
    attributed_string.replace_str(&cf_string, CFRange::init(0, 0));
    
    // Note: Font will be set automatically by Core Text based on the text content
    // ⭐⭐⭐⭐⭐ 必须在这里
    let font = create_base_font(font_size);

    // Set the font attribute - need to get the font key as CFString
    unsafe {
        // Get the font key as a CFString
        let font_key = if !kCTFontAttributeName.is_null() {
            CFString::wrap_under_get_rule(kCTFontAttributeName as *const _)
        } else {
            // Fallback: create the key manually
            CFString::new("NSFont")
        };
        
        // Get the length of the attributed string (in UTF-16 code units)
        // After replace_str, the attributed string length matches the CFString length
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFAttributedStringGetLength(aStr: *const c_void) -> isize;
        }
        let text_length = CFAttributedStringGetLength(attributed_string.as_concrete_TypeRef() as *const c_void);
        
        CFAttributedStringSetAttribute(
            attributed_string.as_concrete_TypeRef() as *mut c_void,
            CFRange::init(0, text_length as i64),
            font_key.as_concrete_TypeRef() as *const c_void,
            font.as_concrete_TypeRef() as *const c_void,
        );
    }

    // The font pointer will be retrieved from each run's attributes
    
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
                
                let font_ptr = if !attributes_dict.is_null() && !kCTFontAttributeName.is_null() {
                    // Get font key
                    let font_key_cf = CFString::wrap_under_get_rule(kCTFontAttributeName as *const _);
                    
                    // Get the font value from the attributes dictionary
                    let font_value_ref = CFDictionaryGetValue(
                        attributes_dict,
                        font_key_cf.as_concrete_TypeRef() as *const c_void,
                    );
                    
                    if font_value_ref.is_null() {
                        ptr::null()
                    } else {
                        font_value_ref
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

fn main() {
    let test_string = "Hello, Java; 世界;! 🌍";
    // Use the public function from lib.rs
    core_text_runs::split_str_into_runs_impl(test_string, 16.0);
}
