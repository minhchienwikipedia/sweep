// Renders the DMG installer window background.
// Usage: swift scripts/dmg-background.swift <out.png> <scale>
import AppKit

let args = CommandLine.arguments
let outPath = args[1]
let scale = CGFloat(Double(args[2]) ?? 1)
let size = CGSize(width: 640, height: 400)

let rep = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: Int(size.width * scale), pixelsHigh: Int(size.height * scale),
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
rep.size = size

NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)

// Soft vertical gradient
NSGradient(colors: [
    NSColor(calibratedRed: 0.97, green: 0.96, blue: 1.0, alpha: 1),
    NSColor(calibratedRed: 0.90, green: 0.89, blue: 0.97, alpha: 1),
])!.draw(in: NSRect(origin: .zero, size: size), angle: -90)

// Title
let title = NSAttributedString(string: "Sweep", attributes: [
    .font: NSFont.systemFont(ofSize: 26, weight: .semibold),
    .foregroundColor: NSColor(calibratedWhite: 0.15, alpha: 1),
])
title.draw(at: NSPoint(x: (size.width - title.size().width) / 2, y: size.height - 62))

let subtitle = NSAttributedString(string: "Drag the app into Applications to install", attributes: [
    .font: NSFont.systemFont(ofSize: 13, weight: .regular),
    .foregroundColor: NSColor(calibratedWhite: 0.4, alpha: 1),
])
subtitle.draw(at: NSPoint(x: (size.width - subtitle.size().width) / 2, y: size.height - 88))

// Arrow between the two icon slots (icons centred at x=170 and x=470, y=210 from top)
let arrowY = size.height - 210
let arrow = NSBezierPath()
arrow.move(to: NSPoint(x: 265, y: arrowY))
arrow.line(to: NSPoint(x: 360, y: arrowY))
arrow.lineWidth = 6
arrow.lineCapStyle = .round
let accent = NSColor(calibratedRed: 0.45, green: 0.38, blue: 0.85, alpha: 0.85)
accent.setStroke()
arrow.stroke()

let head = NSBezierPath()
head.move(to: NSPoint(x: 378, y: arrowY))
head.line(to: NSPoint(x: 352, y: arrowY + 16))
head.line(to: NSPoint(x: 352, y: arrowY - 16))
head.close()
accent.setFill()
head.fill()

NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: outPath))
