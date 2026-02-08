import SwiftUI

struct ContentView: View {
    @State private var output: String = "Tap 'Run Analysis' to see font runs..."
    
    var body: some View {
        VStack(spacing: 20) {
            Text("Core Text Runs")
                .font(.largeTitle)
                .padding()
            
            Button("Run Analysis") {
                runAnalysis()
            }
            .padding()
            .buttonStyle(.borderedProminent)
            
            ScrollView {
                Text(output)
                    .font(.system(.body, design: .monospaced))
                    .padding()
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(maxWidth: .infinity)
        }
        .padding()
    }
    
    func runAnalysis() {
        let testString = "Hello, Java; 世界;! 💇‍♀️🌍"
        
        // Create output string
        var result = "Number of lines: 1\n"
        result += "Text: \"\(testString)\"\n"
        result += "---\n"
        
        // Call Rust function and capture output
        testString.withCString { cString in
            // Redirect stdout temporarily
            let pipe = Pipe()
            let fileHandle = pipe.fileHandleForReading
            
            // Save original stdout
            let originalStdout = dup(STDOUT_FILENO)
            
            // Redirect stdout to pipe
            dup2(pipe.fileHandleForWriting.fileDescriptor, STDOUT_FILENO)
            close(pipe.fileHandleForWriting.fileDescriptor)
            
            // Call Rust function
            split_str_into_runs(cString, 16.0)
            
            split_and_shape_text(cString, 16.0)
            
            // Flush and restore stdout
            fflush(stdout)
            dup2(originalStdout, STDOUT_FILENO)
            close(originalStdout)
            
            // Read output
            let data = fileHandle.readDataToEndOfFile()
            if let output = String(data: data, encoding: .utf8) {
                DispatchQueue.main.async {
                    self.output = output
                }
            }
        }
    }
}

#Preview {
    ContentView()
}
