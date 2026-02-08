# Running Core Text Runs in iOS Simulator

This project contains Rust code that uses Core Text to split strings into font runs. To run it in the iOS simulator:

## Quick Start

1. **Build the Rust library:**
   ```bash
   cargo build --target aarch64-apple-ios-sim --lib
   ```

2. **Create an iOS app in Xcode:**
   - Open Xcode
   - Create a new iOS App project (SwiftUI)
   - Name it "CoreTextRuns"

3. **Add the Rust library:**
   - Drag `target/aarch64-apple-ios-sim/debug/libcore_text_runs.a` into your Xcode project
   - Make sure "Copy items if needed" is checked

4. **Create a bridging header:**
   - Add a new file: `CoreTextRuns-Bridging-Header.h`
   - Add this content:
     ```c
     #ifndef CoreTextRuns_Bridging_Header_h
     #define CoreTextRuns_Bridging_Header_h
     
     extern void split_str_into_runs(const char *text, double font_size);
     
     #endif
     ```
   - In Build Settings, set "Objective-C Bridging Header" to: `CoreTextRuns/CoreTextRuns-Bridging-Header.h`

5. **Link frameworks:**
   - In Build Phases → Link Binary With Libraries, add:
     - CoreText.framework
     - CoreFoundation.framework
     - CoreGraphics.framework

6. **Update ContentView.swift:**
   ```swift
   import SwiftUI
   
   struct ContentView: View {
       @State private var output: String = "Tap to run..."
       
       var body: some View {
           VStack {
               Text("Core Text Runs")
                   .font(.largeTitle)
               
               Button("Run Analysis") {
                   let testString = "Hello, Java; 世界;! 🌍"
                   testString.withCString { cString in
                       split_str_into_runs(cString, 16.0)
                   }
               }
               
               ScrollView {
                   Text(output)
                       .font(.system(.body, design: .monospaced))
               }
           }
           .padding()
       }
   }
   ```

7. **Run in simulator:**
   - Select an iOS simulator (e.g., iPhone 15)
   - Click Run (⌘R)

## Alternative: Use Pre-created Structure

The `ios-app/` directory contains a basic SwiftUI app structure. You can:
1. Open it in Xcode (if the .xcodeproj exists)
2. Or copy the Swift files into your own Xcode project

## Library Location

The built library is at:
```
target/aarch64-apple-ios-sim/debug/libcore_text_runs.a
```

## Function Signature

The Rust function exposed to Swift:
```c
extern void split_str_into_runs(const char *text, double font_size);
```

This function will print the font run analysis to stdout, which will appear in Xcode's console.
