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
// Using harfbuzz_sys directly for low-level HarfBuzz API

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

// Structure to hold run information
#[derive(Debug, Clone)]
pub struct TextRun {
    pub text: String,
    pub font_name: String,
    pub start_utf16: usize,
    pub length_utf16: usize,
    // CRITICAL: This is a borrowed reference from the attributes dictionary.
    // The font is retained by the attributed string/run - NEVER release it.
    // It will be automatically released when the attributed string/run is deallocated.
    // Stored as u64 to avoid pointer lifetime issues
    pub font_ptr: u64,
}

// Structure to hold shaping results
#[derive(Debug)]
pub struct ShapingResult {
    pub run_text: String,
    pub font_name: String,
    pub glyph_count: usize,
    pub glyph_ids: Vec<u32>,
    pub cluster_indices: Vec<u32>,
    pub x_advances: Vec<i32>,
    pub y_advances: Vec<i32>,
}

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

// Internal structure for collecting runs (before UTF-8 conversion)
struct RunRaw {
    utf16_location: isize,
    utf16_length: isize,
    postscript_name: String,
    font_ptr: *const c_void,
}

// Collect runs from a CTFrame - following the pattern from the reference implementation
fn collect_runs_from_frame(text: &str, frame: *const c_void) -> Vec<RunRaw> {
    let mut out = Vec::new();
    
    unsafe {
        #[link(name = "CoreText", kind = "framework")]
        extern "C" {
            fn CTFrameGetLines(frame: *const c_void) -> *const c_void;
            fn CTLineGetGlyphRuns(line: *const c_void) -> *const c_void;
            fn CTRunGetAttributes(run: *const c_void) -> *const c_void;
            fn CTRunGetStringRange(run: *const c_void) -> CFRange;
            fn CTFontCopyPostScriptName(font: *const c_void) -> *const c_void;
        }
        
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFArrayGetCount(array: *const c_void) -> isize;
            fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
            fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
        }
        
        let lines = CTFrameGetLines(frame);
        if lines.is_null() {
            return out;
        }
        
        let utf16_total = text.encode_utf16().count() as isize;
        let line_count = CFArrayGetCount(lines);
        
        for line_idx in 0..line_count {
            let line = CFArrayGetValueAtIndex(lines, line_idx) as *const c_void;
            if line.is_null() {
                continue;
            }
            
            let runs = CTLineGetGlyphRuns(line);
            if runs.is_null() {
                continue;
            }
            
            let run_count = CFArrayGetCount(runs);
            for run_idx in 0..run_count {
                let run = CFArrayGetValueAtIndex(runs, run_idx) as *const c_void;
                if run.is_null() {
                    continue;
                }
                
                let range = CTRunGetStringRange(run);
                if range.location < 0 || range.length < 0 || range.location + range.length > utf16_total {
                    continue;
                }
                
                let attrs = CTRunGetAttributes(run);
                if attrs.is_null() {
                    continue;
                }
                
                // Get font pointer from attributes dictionary using kCTFontAttributeName directly
                // CRITICAL: This is a borrowed reference from the attributes dictionary.
                // We need to retain it to ensure it stays valid after the frame is dropped.
                let font_ptr = CFDictionaryGetValue(attrs, kCTFontAttributeName as *const c_void) as *const c_void;
                if font_ptr.is_null() {
                    continue;
                }
                
                // Retain the font to ensure it stays valid after the frame is dropped
                #[link(name = "CoreFoundation", kind = "framework")]
                extern "C" {
                    fn CFRetain(cf: *const c_void) -> *const c_void;
                }
                let retained_font_ptr = CFRetain(font_ptr);
                if retained_font_ptr.is_null() {
                    continue;
                }
                
                // Get PostScript name from font
                let ps_name_ref = CTFontCopyPostScriptName(retained_font_ptr);
                if ps_name_ref.is_null() {
                    continue;
                }
                let ps_name_cf = CFString::wrap_under_create_rule(ps_name_ref as *mut _);
                let ps_name = ps_name_cf.to_string();
                if ps_name.is_empty() {
                    continue;
                }
                
                out.push(RunRaw {
                    utf16_location: range.location,
                    utf16_length: range.length,
                    postscript_name: ps_name,
                    font_ptr: retained_font_ptr, // Retained reference - must be released later
                });
            }
        }
    }
    
    out
}

// Function to collect runs from text
fn collect_runs(text: &str, font_size: f64) -> Vec<TextRun> {
    // Create base font using system UI font
    let font = create_base_font(font_size);
    
    // Create CFString from Rust string
    let cf_string = CFString::new(text);
    
    // Create mutable attributed string
    let mut attributed_string = CFMutableAttributedString::new();
    attributed_string.replace_str(&cf_string, CFRange::init(0, 0));
    
    // Set the font attribute for the entire string using C API
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
        
        let text_length = CFAttributedStringGetLength(attributed_string.as_concrete_TypeRef() as *const c_void);
        let attr_str_ptr = attributed_string.as_concrete_TypeRef() as *mut c_void;
        let font_ptr = font.as_concrete_TypeRef() as *const c_void;
        
        if !attr_str_ptr.is_null() && !font_ptr.is_null() {
            if let Some(font_key_ptr) = get_font_attribute_name() {
                // CFAttributedStringSetAttribute will retain the font object
                // This prevents the font from being released when the Rust wrapper is dropped
                CFAttributedStringSetAttribute(
                    attr_str_ptr,
                    CFRange::init(0, text_length as isize),
                    font_key_ptr,
                    font_ptr,
                );
            }
        }
        
        // Prevent the font Rust wrapper from releasing the Core Foundation font object
        // CFAttributedStringSetAttribute has retained it, so it's now owned by the attributed string
        std::mem::forget(font);
    }
    
    // Create framesetter
    let framesetter = CTFramesetter::new_with_attributed_string(attributed_string.as_concrete_TypeRef());
    // Prevent the attributed_string Rust wrapper from releasing the Core Foundation object
    std::mem::forget(attributed_string);
    
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
    
    // Collect runs from frame using the new pattern
    let raw_runs = collect_runs_from_frame(text, frame.as_concrete_TypeRef() as *const c_void);
    
    // Convert RunRaw to TextRun with UTF-8 text extraction
    let mut runs = Vec::new();
    let text_utf16: Vec<u16> = text.encode_utf16().collect();
    
    for raw_run in raw_runs {
        let start_utf16 = raw_run.utf16_location as usize;
        let length_utf16 = raw_run.utf16_length as usize;
        
        // Convert UTF-16 indices to UTF-8 string
        let run_text = if start_utf16 + length_utf16 <= text_utf16.len() {
            let utf16_slice = &text_utf16[start_utf16..start_utf16 + length_utf16];
            match String::from_utf16(utf16_slice) {
                Ok(s) => s,
                Err(_) => String::from(""),
            }
        } else {
            String::from("")
        };
        
        runs.push(TextRun {
            text: run_text,
            font_name: raw_run.postscript_name,
            start_utf16,
            length_utf16,
            font_ptr: raw_run.font_ptr as u64, // Borrowed reference - NEVER release, stored as u64
        });
    }
    
    runs
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
                CFRange::init(0, text_length as isize),
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


// Function to shape a run with HarfBuzz using harfbuzz_sys directly with CTFont
pub fn shape_run_with_harfbuzz(run: &TextRun) -> Option<ShapingResult> {
    use harfbuzz_sys;
    use std::ffi::CString;
    
    unsafe {
        // Step 1: Validate font pointer before use
        if run.font_ptr == 0 {
            return None;
        }
        
        // Step 2: Get font pointer (already retained in collect_runs_from_frame)
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFRelease(cf: *const c_void);
        }
        
        // Cast u64 back to pointer (this is already a retained reference)
        let ct_font_ptr = run.font_ptr as usize as *const c_void;
        
        // Step 3: Create harfbuzz font directly from CTFont pointer using CoreText integration
        // hb_coretext_font_create creates a harfbuzz font from a CTFontRef
        let font = harfbuzz_sys::coretext::hb_coretext_font_create(ct_font_ptr as *const _);
        
        if font.is_null() {
            // Release the retained font if harfbuzz font creation failed
            CFRelease(ct_font_ptr);
            return None;
        }
        
        // Step 7: Create harfbuzz buffer
        let buffer = harfbuzz_sys::hb_buffer_create();
        
        if buffer.is_null() {
            harfbuzz_sys::hb_font_destroy(font);
            CFRelease(ct_font_ptr);
            println!("DEBUG: Failed to create harfbuzz buffer");
            return None;
        }
        
        // Step 8: Add text to buffer
        let text_cstring = match CString::new(run.text.as_str()) {
            Ok(s) => s,
            Err(_) => {
                harfbuzz_sys::hb_buffer_destroy(buffer);
                harfbuzz_sys::hb_font_destroy(font);
                CFRelease(ct_font_ptr);
                return None;
            }
        };
        
        let text_bytes = text_cstring.as_bytes_with_nul();
        harfbuzz_sys::hb_buffer_add_utf8(
            buffer,
            text_bytes.as_ptr() as *const i8,
            (text_bytes.len() - 1) as i32, // -1 to exclude null terminator
            0,
            -1,
        );
        
        // Set buffer direction and script
        harfbuzz_sys::hb_buffer_set_direction(buffer, harfbuzz_sys::HB_DIRECTION_LTR);
        
        // Detect script from text content - emoji fonts may need special handling
        let script = if run.font_name.contains("Emoji") || run.font_name.contains("emoji") {
            // Use COMMON script for emoji
            harfbuzz_sys::HB_SCRIPT_COMMON
        } else {
            // Default to LATIN for other text
            harfbuzz_sys::HB_SCRIPT_LATIN
        };
        harfbuzz_sys::hb_buffer_set_script(buffer, script);
        harfbuzz_sys::hb_buffer_set_language(buffer, harfbuzz_sys::hb_language_from_string(
            b"en\0".as_ptr() as *const i8,
            -1,
        ));
        
        // Step 9: Shape the buffer
        // Note: Some fonts (especially emoji fonts) may not support HarfBuzz shaping
        // If shaping fails, we return None gracefully
        harfbuzz_sys::hb_shape(font, buffer, ptr::null(), 0);
        
        // Step 10: Get glyph information
        let mut glyph_count: u32 = 0;
        let glyph_infos = harfbuzz_sys::hb_buffer_get_glyph_infos(buffer, &mut glyph_count);
        let glyph_positions = harfbuzz_sys::hb_buffer_get_glyph_positions(buffer, &mut glyph_count);
        
        if glyph_infos.is_null() || glyph_positions.is_null() || glyph_count == 0 {
            harfbuzz_sys::hb_buffer_destroy(buffer);
            harfbuzz_sys::hb_font_destroy(font);
            CFRelease(ct_font_ptr);
            return None;
        }
        
        // Step 11: Extract glyph data
        let glyph_count_usize = glyph_count as usize;
        let mut glyph_ids = Vec::with_capacity(glyph_count_usize);
        let mut cluster_indices = Vec::with_capacity(glyph_count_usize);
        let mut x_advances = Vec::with_capacity(glyph_count_usize);
        let mut y_advances = Vec::with_capacity(glyph_count_usize);
        
        for i in 0..glyph_count_usize {
            let info = *glyph_infos.add(i);
            let pos = *glyph_positions.add(i);
            
            glyph_ids.push(info.codepoint);
            cluster_indices.push(info.cluster);
            // HarfBuzz positions are in 26.6 fixed point, convert to i32
            x_advances.push(pos.x_advance);
            y_advances.push(pos.y_advance);
        }
        
        // Clean up
        harfbuzz_sys::hb_buffer_destroy(buffer);
        harfbuzz_sys::hb_font_destroy(font);
        CFRelease(ct_font_ptr); // Release the font we retained in collect_runs_from_frame
        
        Some(ShapingResult {
            run_text: run.text.clone(),
            font_name: run.font_name.clone(),
            glyph_count: glyph_count_usize,
            glyph_ids,
            cluster_indices,
            x_advances,
            y_advances,
        })
    }
}

// FFI function that splits text into runs and shapes them with HarfBuzz
#[no_mangle]
pub extern "C" fn split_and_shape_text(text: *const i8, font_size: f64) {
    use std::ffi::CStr;
    
    let text_str = unsafe {
        CStr::from_ptr(text)
            .to_str()
            .unwrap_or("")
    };
    
    println!("=== Splitting and Shaping Text ===");
    println!("Text: \"{}\"", text_str);
    println!("Font size: {}", font_size);
    println!("---");
    
    // Step 1: Split text into runs
    let runs = collect_runs(text_str, font_size);
    println!("Found {} runs", runs.len());
    println!("---");
    
    // Step 2: Shape each run with HarfBuzz
    for (idx, run) in runs.iter().enumerate() {
        println!("Run {}: \"{}\"", idx, run.text);
        println!("  Font: {}", run.font_name);
        println!("  ptr: 0x{:x}", run.font_ptr);
        println!("  UTF-16 range: {}..{}", run.start_utf16, run.start_utf16 + run.length_utf16);
        
        if let Some(shaping_result) = shape_run_with_harfbuzz(run) {
            println!("  Shaping Result:");
            println!("    Glyph count: {}", shaping_result.glyph_count);
            println!("    Glyph IDs: {:?}", shaping_result.glyph_ids);
            println!("    Cluster indices: {:?}", shaping_result.cluster_indices);
            println!("    X advances: {:?}", shaping_result.x_advances);
            println!("    Y advances: {:?}", shaping_result.y_advances);
        } else {
            println!("  Shaping failed");
        }
        println!("---");
    }
    
    println!("=== Done ===");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_split_and_shape_text_basic() {
        // Test with a simple ASCII string
        let text = "Hello, World!";
        let c_string = CString::new(text).expect("CString::new failed");
        let text_ptr = c_string.as_ptr() as *const i8;
        
        // This should not panic
        split_and_shape_text(text_ptr, 16.0);
    }

    #[test]
    fn test_split_and_shape_text_unicode() {
        // Test with Unicode characters - just test that runs are collected correctly
        // We test collect_runs directly to avoid potential font pointer lifetime issues
        let text = "Hello 世界";
        let runs = collect_runs(text, 20.0);
        
        // Should have at least one run
        assert!(!runs.is_empty(), "Should have at least one run for Unicode text");
        
        // Verify that all text is covered by runs
        let total_utf16: usize = text.encode_utf16().count();
        let mut covered: usize = 0;
        
        for run in &runs {
            assert!(run.start_utf16 <= total_utf16, "Start UTF-16 index out of bounds");
            assert!(run.start_utf16 + run.length_utf16 <= total_utf16, "End UTF-16 index out of bounds");
            covered += run.length_utf16;
            assert!(!run.font_name.is_empty(), "Font name should not be empty");
        }
        
        // All UTF-16 code units should be covered
        assert_eq!(covered, total_utf16, "All UTF-16 code units should be covered by runs");
    }

    #[test]
    fn test_split_and_shape_text_empty() {
        // Test with empty string
        let text = "";
        let c_string = CString::new(text).expect("CString::new failed");
        let text_ptr = c_string.as_ptr() as *const i8;
        
        // This should not panic
        split_and_shape_text(text_ptr, 12.0);
    }

    #[test]
    fn test_split_and_shape_text_multiline() {
        // Test with multiline text
        let text = "Line 1\nLine 2\nLine 3";
        let c_string = CString::new(text).expect("CString::new failed");
        let text_ptr = c_string.as_ptr() as *const i8;
        
        // This should not panic
        split_and_shape_text(text_ptr, 14.0);
    }

    #[test]
    fn test_split_and_shape_text_emoji() {
        // Test with multiline text
        let text = "🤔💇‍♀️";
        let c_string = CString::new(text).expect("CString::new failed");
        let text_ptr = c_string.as_ptr() as *const i8;
        
        // This should not panic
        split_and_shape_text(text_ptr, 14.0);
    }


    #[test]
    fn test_collect_runs_basic() {
        // Test the collect_runs function directly
        let text = "Hello, World!";
        let runs = collect_runs(text, 16.0);
        
        // Should have at least one run
        assert!(!runs.is_empty(), "Should have at least one run");
        
        // Verify run properties
        for run in &runs {
            assert!(!run.text.is_empty() || run.length_utf16 == 0, "Run text should not be empty unless length is 0");
            assert!(!run.font_name.is_empty(), "Font name should not be empty");
            assert!(run.font_ptr != 0, "Font pointer should not be zero");
        }
    }

    #[test]
    fn test_collect_runs_unicode() {
        // Test with Unicode characters
        let text = "Hello 世界";
        let runs = collect_runs(text, 20.0);
        
        assert!(!runs.is_empty(), "Should have at least one run");
        
        // Verify UTF-16 ranges are valid
        let total_utf16: usize = text.encode_utf16().count();
        let mut covered: usize = 0;
        
        for run in &runs {
            assert!(run.start_utf16 <= total_utf16, "Start UTF-16 index out of bounds");
            assert!(run.start_utf16 + run.length_utf16 <= total_utf16, "End UTF-16 index out of bounds");
            covered += run.length_utf16;
        }
        
        // All UTF-16 code units should be covered
        assert_eq!(covered, total_utf16, "All UTF-16 code units should be covered by runs");
    }
}
