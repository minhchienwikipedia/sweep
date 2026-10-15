// Sets a custom Finder icon on a file.
// Usage: swift scripts/set-file-icon.swift <icon.icns> <file>
import AppKit

let args = CommandLine.arguments
guard let icon = NSImage(contentsOfFile: args[1]),
      NSWorkspace.shared.setIcon(icon, forFile: args[2], options: []) else {
    FileHandle.standardError.write("Failed to set icon on \(args[2])\n".data(using: .utf8)!)
    exit(1)
}
