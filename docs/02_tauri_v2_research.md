# Tauri v2 Research: Screen Capture Tool Feasibility (macOS & Windows)

> Date: 2026-03-25
> Purpose: Evaluate Tauri v2 as an alternative to Swift/SwiftUI for Gaze

---

## 1. Tauri v2 Current State (2026)

### 1.1 Version & Stability

| Item | Details |
|------|---------|
| **Latest Version** | Tauri v2.10.3 (stable) |
| **Stable Release Date** | October 2, 2024 |
| **Maturity** | 1.5+ years of stable v2 releases with regular patches |
| **Related Components** | tauri-bundler v2.8.1, wry v0.54.4, tao v0.34.6 |
| **Mobile Support** | iOS and Android added in v2 |

Tauri v2 is production-ready and actively maintained with frequent patch releases. The ecosystem is mature enough for production desktop apps.

### 1.2 Performance Benchmarks vs Electron

| Metric | Tauri v2 | Electron | Difference |
|--------|----------|----------|------------|
| **App Installer Size** | ~2.5-10 MB | ~80-120 MB | **10-30x smaller** |
| **Startup Time** | 0.5-1 sec | 2-4 sec | **~3x faster** |
| **Memory Usage (idle)** | 50-150 MB | 200-500 MB | **~3x less** |
| **CPU at Idle** | 0-1% | 2-5% | **Significantly lower** |
| **Real-time Data Processing** | Baseline | +40% startup, +30% memory | Per 2026 benchmark |

### 1.3 Architecture Difference

- **Tauri**: Uses OS-native WebView (WKWebView on macOS, WebView2 on Windows). No bundled browser engine. Backend in Rust.
- **Electron**: Bundles full Chromium + Node.js. Backend in JavaScript/Node.js.

Sources:
- [Tauri 2.0 Stable Release](https://v2.tauri.app/blog/tauri-20/)
- [Tauri Core Ecosystem Releases](https://v2.tauri.app/release/)
- [GitHub Releases](https://github.com/tauri-apps/tauri/releases)
- [Tauri vs Electron - Real World Application](https://www.levminer.com/blog/tauri-vs-electron)
- [Tauri vs Electron Complete Guide 2026](https://blog.nishikanta.in/tauri-vs-electron-the-complete-developers-guide-2026)
- [Tauri v2 vs Electron Comparison - Oflight](https://www.oflight.co.jp/en/columns/tauri-v2-vs-electron-comparison)
- [Hopp Blog - Tauri vs Electron](https://www.gethopp.app/blog/tauri-vs-electron)
- [Tauri vs Electron - Rust's Approach](https://dasroot.net/posts/2026/03/tauri-vs-electron-rust-cross-platform-apps/)

---

## 2. Tauri v2 for Screen Capture Specifically

### 2.1 ScreenCaptureKit on macOS

**YES** - Accessible via Rust FFI through multiple crates:

| Crate | Description | Status |
|-------|-------------|--------|
| **[screencapturekit](https://crates.io/crates/screencapturekit)** | Safe, idiomatic Rust bindings for Apple's ScreenCaptureKit | Active, v1.5.0. Has a dedicated Tauri example (`examples/22_tauri_app`) |
| **[screen-capture-kit](https://crates.io/crates/screen-capture-kit)** | Alternative Rust bindings for ScreenCaptureKit | Available |
| **[scap](https://github.com/CapSoftware/scap)** | Cross-platform capture library (uses ScreenCaptureKit on macOS) | Active, by Cap team |
| **[screenshots](https://crates.io/crates/screenshots)** | Cross-platform screenshot library | Simpler API, less control |

The `screencapturekit` crate supports:
- Capturing screen content, windows, and applications
- Both synchronous and asynchronous APIs
- Metal GPU integration
- Audio capture
- macOS 12.3+
- **Tauri integration example included**

### 2.2 Windows Screen Capture APIs

**YES** - Accessible via Rust crates:

| Crate | Description | Status |
|-------|-------------|--------|
| **[windows-capture](https://github.com/NiiightmareXD/windows-capture)** | Uses Windows.Graphics.Capture API. "Fastest Windows Screen Capture Library for Rust" | Active, v1.5.0 |
| **[scap](https://github.com/CapSoftware/scap)** | Uses Windows.Graphics.Capture on Windows | Active |
| **[screenshots](https://crates.io/crates/screenshots)** | Cross-platform (Windows, macOS, Linux) | Simpler |

`windows-capture` features:
- Frame-by-frame capture with event handling
- Video encoding support
- Cursor capture options
- High performance, only updates frames when needed

### 2.3 System Tray / Menu Bar Apps

**Fully Supported** via `tauri-plugin-system-tray`:
- Native tray icon with custom menus
- Action items, checkboxes, submenus, separators
- Left/right click handling
- Menu positioning near tray icon
- Window show/hide on tray click

Reference: [System Tray - Tauri v2](https://v2.tauri.app/learn/system-tray/)

### 2.4 Global Hotkey Support

**Fully Supported** via `tauri-plugin-global-shortcut`:
- Register global shortcuts from both JavaScript and Rust
- Supports modifier combinations (Ctrl, Alt, Shift, Command)
- Key press and release events
- Multiple shortcut registration at once

```rust
// Rust example
tauri_plugin_global_shortcut::Builder::new()
    .with_shortcuts(["ctrl+shift+c", "alt+space"])
    .with_handler(|app, shortcut, event| { /* ... */ })
```

```javascript
// JavaScript example
await register('CommandOrControl+Shift+C', (event) => {
    if (event.state === "Pressed") { /* ... */ }
});
```

**Caveat**: If a shortcut is already registered by another application, the handler will not trigger.

References:
- [Global Shortcut Plugin](https://v2.tauri.app/plugin/global-shortcut/)
- [global-hotkey crate](https://github.com/tauri-apps/global-hotkey)

### 2.5 Overlay Window (Capture Selection Area)

**Partially Supported** with caveats:

Supported features:
- `alwaysOnTop: true` - Window stays above other windows
- `transparent: true` - Transparent background
- `decorations: false` - No title bar/borders
- `skipTaskbar: true` - Hidden from taskbar
- Fullscreen overlay possible

Known issues:
- **Transparent window inconsistency**: v1 and v2 behave differently on Windows. Some reports of transparent windows not working correctly in v2 on Windows
- **Click-through transparency**: Not natively supported yet (feature request exists: [Issue #13070](https://github.com/tauri-apps/tauri/issues/13070))
- **White flash on show**: Hidden transparent windows may flash white when first displayed ([Issue #14515](https://github.com/tauri-apps/tauri/issues/14515))

**Assessment**: The overlay for capture area selection is feasible but requires careful implementation, especially on Windows. The approach would be:
1. Create fullscreen transparent window on capture trigger
2. Render selection rectangle via web frontend (HTML Canvas / SVG)
3. Capture coordinates, close overlay, then capture via Rust backend

References:
- [Window Customization](https://v2.tauri.app/learn/window-customization/)
- [tauri-plugin-decorum](https://crates.io/crates/tauri-plugin-decorum)

### 2.6 Tauri Screenshot Plugin

**[tauri-plugin-screenshots](https://crates.io/crates/tauri-plugin-screenshots)**: A ready-made Tauri v2 plugin that uses the `xcap` library for capturing windows and monitors. Could serve as a starting point or fallback.

Sources:
- [screencapturekit on crates.io](https://crates.io/crates/screencapturekit)
- [screencapturekit-rs GitHub](https://github.com/svtlabs/screencapturekit-rs)
- [windows-capture GitHub](https://github.com/NiiightmareXD/windows-capture)
- [scap GitHub](https://github.com/CapSoftware/scap)
- [tauri-plugin-screenshots](https://crates.io/crates/tauri-plugin-screenshots)
- [Tauri v2 Multi-Window and System Tray Guide](https://www.oflight.co.jp/en/columns/tauri-v2-multi-window-system-tray)

---

## 3. Tauri + Rust Ecosystem for This Use Case

### 3.1 Image Processing

| Crate | Purpose | Notes |
|-------|---------|-------|
| **[image](https://crates.io/crates/image)** | Core image processing (resize, crop, format conversion) | Supports JPEG, PNG, GIF, WebP, BMP, TIFF. The de facto standard |
| **[imageproc](https://crates.io/crates/imageproc)** | Advanced processing (drawing, filtering, transformations) | Built on `image` crate |
| **[ril](https://github.com/jay3332/ril)** | High-level imaging (including animated images) | Claimed faster than image-rs for some operations |
| **[fast_image_resize](https://crates.io/crates/fast_image_resize)** | SIMD-accelerated image resizing | For high-performance resize operations |
| **[webp](https://crates.io/crates/webp)** | WebP encoding/decoding | For LLM-optimized output (Claude prefers WebP) |

The `image` crate supports all formats needed for LLM optimization:
- JPEG, PNG, WebP encoding with quality control
- Resize with various filters (Lanczos3, CatmullRom, etc.)
- Crop, rotate, color conversion

### 3.2 GIF Encoding

| Crate | Purpose | Notes |
|-------|---------|-------|
| **[gif](https://crates.io/crates/gif)** | GIF encoding/decoding | Core GIF support, LZW compression |
| **[image](https://crates.io/crates/image)** | GIF support via image crate | Higher-level API |
| **[ril](https://github.com/jay3332/ril)** | Animated GIF support | High-level API for animated images |
| **[gifski](https://gif.ski/)** | High-quality GIF encoding | pngquant-based, best quality GIF encoder (used by many apps) |

For Gaze's GitHub-optimized GIF output, `gifski` would produce the best quality with optimal file sizes (256-color palette optimization, dithering).

### 3.3 OCR Capabilities

| Crate | Platform | Notes |
|-------|----------|-------|
| **[uni-ocr](https://crates.io/crates/uni-ocr)** | macOS (Vision), Windows (native), + Tesseract | **Best choice** - unified API, uses native OCR on each platform. No additional setup on macOS |
| **[tesseract-rs](https://crates.io/crates/tesseract-rs)** | Cross-platform | Rust bindings for Tesseract. Requires Tesseract installation |
| **[leptess](https://github.com/houqp/leptess)** | Cross-platform | Safe bindings for Leptonica + Tesseract |

**`uni-ocr` is the recommended choice** because:
- On macOS: Uses Apple Vision framework natively (no additional setup)
- On Windows: Uses Windows native OCR
- Fallback to Tesseract if needed
- Single API across platforms

### 3.4 Clipboard Management

| Plugin/Crate | Features |
|-------------|----------|
| **[tauri-plugin-clipboard-manager](https://v2.tauri.app/plugin/clipboard/)** (official) | Text and image write to clipboard. Official Tauri v2 plugin |
| **[tauri-plugin-clipboard](https://github.com/CrossCopy/tauri-plugin-clipboard)** (CrossCopy) | Text, HTML, RTF, image, and files. Clipboard content monitoring. More feature-rich |

Both support writing images to the clipboard, which is essential for the "capture -> clipboard -> paste to LLM" workflow.

The CrossCopy plugin additionally supports:
- Clipboard change monitoring (useful for burst mode)
- Multiple format types (HTML, RTF, files)
- Base64 image writing

Sources:
- [image crate docs](https://docs.rs/image/latest/image/)
- [gif crate](https://crates.io/crates/gif)
- [uni-ocr crate](https://crates.io/crates/uni-ocr)
- [Tauri Clipboard Plugin](https://v2.tauri.app/plugin/clipboard/)
- [CrossCopy Clipboard Plugin](https://github.com/CrossCopy/tauri-plugin-clipboard)

---

## 4. Real-World Tauri Apps for Screen Capture

### 4.1 Cap (Open Source Loom Alternative) -- THE KEY REFERENCE

**[Cap](https://github.com/CapSoftware/Cap)** is the most significant proof-of-concept for Tauri + screen capture:

| Aspect | Details |
|--------|---------|
| **What** | Open source Loom alternative for screen recording |
| **Stack** | Tauri v2 + Rust + SolidStart (frontend) |
| **Platforms** | macOS and Windows |
| **Features** | 4K@60fps capture, AI transcription, shareable links, cloud + local storage |
| **Architecture** | Turborepo monorepo: Tauri desktop app + Next.js web app |
| **Screen Capture** | Uses their own `scap` crate (ScreenCaptureKit on macOS, Windows.Graphics.Capture on Windows) |
| **License** | Open source (AGPL) |
| **Status** | Production app, actively used, last updated March 2026 |

**Key Takeaway**: Cap proves that a production-quality screen recording app can be built with Tauri v2 + Rust screen capture crates. They created the `scap` crate specifically because existing Rust screen capture libraries were insufficient.

### 4.2 Other Tauri Screen Capture Projects

| Project | Description | Status |
|---------|-------------|--------|
| **[tauri-screen-recorder](https://github.com/AbhinavRobinson/tauri-screen-recorder)** | WIP screen recorder in Tauri | Experimental |
| **[tauri-site-screenshot](https://github.com/The-Best-Codes/tauri-site-screenshot)** | Website screenshot app with Tauri 2.0 | Example/demo |
| **[tauri-plugin-screenshots](https://crates.io/crates/tauri-plugin-screenshots)** | Plugin using xcap for window/monitor capture | Tauri v2 compatible |

### 4.3 Challenges Faced by Real Projects

Based on Cap and other projects:

1. **Cross-platform capture abstraction**: Cap had to build `scap` because no single crate handled both macOS and Windows well
2. **Permission handling**: macOS Screen Recording permission requires careful UX (especially with Sequoia's weekly re-prompts)
3. **Transparent overlay windows**: Platform inconsistencies require per-OS workarounds
4. **Video encoding performance**: Needed careful Rust optimization for real-time encoding
5. **WebView rendering differences**: Frontend behaves slightly differently between WKWebView and WebView2

Sources:
- [Cap GitHub](https://github.com/CapSoftware/Cap)
- [Cap Website](https://cap.so/)
- [Cap as Loom Alternative](https://cap.so/loom-alternative)
- [scap crate](https://github.com/CapSoftware/scap)

---

## 5. Tauri Limitations and Gotchas

### 5.1 What Tauri CAN'T Do That Electron Can

| Limitation | Impact | Workaround |
|-----------|--------|-----------|
| **No Node.js runtime** | Can't use npm packages directly in backend | Use Rust crates, or run Node as sidecar binary |
| **No consistent WebView API** | Web APIs vary by platform (WebGPU, etc.) | Test on all platforms; avoid cutting-edge web APIs |
| **No Chromium DevTools in production** | Uses private APIs on macOS, can't ship with App Store | Use release builds without devtools |
| **Smaller ecosystem** | Fewer plugins/community resources than Electron | Growing rapidly; Rust ecosystem compensates |
| **Steeper learning curve** | Rust backend vs JavaScript backend | Trade-off for performance gains |
| **No consistent browser engine** | Can't guarantee exact rendering across platforms | WKWebView (macOS) vs WebView2 (Windows) differences are usually minor |

### 5.2 macOS-Specific Limitations

| Issue | Details |
|-------|---------|
| **Screen Recording Permission** | Required for all capture. Sequoia prompts weekly. UX must guide users through this |
| **App Store sandbox** | ScreenCaptureKit requires entitlements that may conflict with App Store sandbox. Direct distribution recommended |
| **Private API usage** | DevTools and transparent backgrounds use private APIs -- cannot ship to App Store with these |
| **WKWebView quirks** | Safari-engine-based; some CSS/JS features may differ from Chromium |
| **WebView version tied to OS** | Users on older macOS get older WebKit. Cannot update independently |

### 5.3 Windows-Specific Limitations

| Issue | Details |
|-------|---------|
| **WebView2 dependency** | Pre-installed on Windows 11 but may need installation on Windows 10. Tauri installer handles this |
| **Transparent window bugs** | v2 has reported issues with transparent windows not working correctly on Windows |
| **Windows.Graphics.Capture availability** | Requires Windows 10 version 1903+ |
| **WinRT async model** | Windows capture APIs use async COM, which requires careful Rust integration |

### 5.4 WebView Differences (WKWebView vs WebView2)

| Aspect | WKWebView (macOS) | WebView2 (Windows) |
|--------|-------------------|-------------------|
| **Engine** | WebKit (Safari) | Chromium (Edge) |
| **Update Mechanism** | OS updates only | Auto-updates with Edge |
| **CSS Compatibility** | Safari-level | Chrome-level |
| **JavaScript Engine** | JavaScriptCore | V8 |
| **WebGPU** | Limited | Available |
| **DevTools** | Requires private API | Available via debug flag |

**Practical impact**: For a screenshot/annotation tool, the WebView differences are **minimal**. The UI is relatively simple (menus, settings, annotation canvas), and all needed web APIs (Canvas, SVG, CSS) work consistently across both engines.

### 5.5 Tauri v2 IPC Improvements

Tauri v2 significantly improved IPC performance over v1:
- v1 used string serialization (slow)
- v2 uses custom protocols, similar to HTTP-based communication
- Much faster for passing image data between Rust backend and web frontend

Sources:
- [Webview Versions Reference](https://v2.tauri.app/reference/webview-versions/)
- [Electron vs Tauri - DoltHub](https://www.dolthub.com/blog/2025-11-13-electron-vs-tauri/)
- [Hopp Blog - Real Trade-offs](https://www.gethopp.app/blog/tauri-vs-electron)
- [Transparent Window Bug #8308](https://github.com/tauri-apps/tauri/issues/8308)
- [Click-Through Feature Request #13070](https://github.com/tauri-apps/tauri/issues/13070)

---

## 6. Feasibility Assessment: Tauri v2 for Gaze

### 6.1 Feature Feasibility Matrix

| Gaze Feature | Tauri Feasibility | Implementation Path |
|-------------------|-------------------|-------------------|
| Area Capture | **Possible** | Transparent overlay window + screencapturekit/windows-capture |
| Window Capture | **Possible** | screencapturekit/scap crate |
| LLM Optimization | **Possible** | `image` crate for resize/compress/format conversion |
| Clipboard Auto-Copy | **Possible** | tauri-plugin-clipboard-manager (images supported) |
| Menu Bar App | **Possible** | tauri-plugin-system-tray |
| Global Hotkeys | **Possible** | tauri-plugin-global-shortcut |
| Basic Annotation | **Possible** | HTML Canvas / SVG in web frontend |
| Smart Crop | **Possible** | `image` + `imageproc` crates |
| OCR Extraction | **Possible** | `uni-ocr` crate (native Vision on macOS, native on Windows) |
| GIF Recording | **Possible** | scap + gif/gifski crates |
| Video Capture | **Possible** | scap + video encoding in Rust |
| Burst Mode | **Possible** | Rust-side multi-capture logic |
| MCP Server | **Possible** | Rust binary, stdio protocol |
| CLI Interface | **Possible** | Rust binary, clap crate |

### 6.2 Tauri vs Swift/SwiftUI Comparison (for Gaze)

| Criterion | Swift/SwiftUI | Tauri v2 (Rust + Web) |
|-----------|-------------|----------------------|
| **macOS Native Feel** | Best | Good (system tray, hotkeys work natively) |
| **ScreenCaptureKit Access** | Direct | Via Rust FFI crate (proven by Cap) |
| **App Size** | 5-15 MB | 2.5-10 MB (actually smaller!) |
| **Memory Usage** | Minimal | 50-150 MB (higher due to WebView) |
| **Cross-Platform** | macOS only | macOS + Windows + Linux |
| **Development Speed** | Moderate (Swift learning curve) | Moderate (Rust learning curve, but web UI is faster) |
| **Annotation UI** | SwiftUI Canvas | HTML Canvas / SVG (larger ecosystem of drawing libraries) |
| **Ecosystem** | Apple frameworks only | Vast Rust + npm ecosystem |
| **Distribution** | Mac App Store or direct | Direct (recommended), or app stores |
| **Windows Support** | Impossible | Built-in |
| **Mobile Support** | iOS via SwiftUI | iOS + Android via Tauri v2 |

### 6.3 Recommendation

**Tauri v2 is viable and has specific advantages over Swift/SwiftUI for Gaze**, particularly:

**Advantages of Tauri approach**:
1. **Cross-platform from day one** -- Windows support opens a much larger market
2. **Proven by Cap** -- A production Tauri v2 screen recording app already exists
3. **Smaller app size** -- Counter-intuitive but true (no framework bundled)
4. **Web-based annotation UI** -- Richer ecosystem for drawing/canvas tools (Fabric.js, Konva, etc.)
5. **Rust performance** -- Image processing pipeline benefits from Rust's speed
6. **Single codebase** -- One team maintains one app for all platforms

**Risks to mitigate**:
1. **Overlay window transparency** on Windows needs careful testing and workarounds
2. **WebView differences** require cross-platform UI testing
3. **Memory overhead** from WebView (50-150MB vs near-zero for native Swift)
4. **macOS permission UX** is harder to make seamless outside native Swift

**Suggested Architecture**:
```
┌─────────────────────────────────────────────┐
│                  Gaze                   │
├─────────────────────────────────────────────┤
│  Tauri v2 Shell                              │
│  ├── System Tray (tauri-plugin-system-tray)  │
│  ├── Global Hotkeys (tauri-plugin-global-shortcut) │
│  ├── Rust Backend                            │
│  │   ├── Capture Engine                      │
│  │   │   ├── macOS: screencapturekit crate   │
│  │   │   └── Windows: windows-capture crate  │
│  │   │   (or scap for unified API)           │
│  │   ├── Image Pipeline (image + imageproc)  │
│  │   ├── GIF Encoder (gifski)                │
│  │   ├── OCR Engine (uni-ocr)                │
│  │   ├── Clipboard (tauri-plugin-clipboard)  │
│  │   └── MCP Server + CLI                    │
│  └── Web Frontend (React/Solid)              │
│      ├── Capture Overlay (Canvas/SVG)        │
│      ├── Annotation Editor (Canvas library)  │
│      ├── Settings UI                         │
│      └── Preview & Export UI                 │
└─────────────────────────────────────────────┘
```

---

## 7. Key Rust Crates Summary

### Screen Capture
| Crate | Platform | Recommended For |
|-------|----------|----------------|
| `scap` | macOS + Windows + Linux | Unified API, by Cap team |
| `screencapturekit` | macOS | Direct ScreenCaptureKit access, more control |
| `windows-capture` | Windows | Direct Windows.Graphics.Capture, best performance |
| `screenshots` / `xcap` | Cross-platform | Simple screenshot-only use cases |

### Image Processing
| Crate | Purpose |
|-------|---------|
| `image` | Core processing, format conversion |
| `imageproc` | Advanced filters, drawing |
| `webp` | WebP encoding for Claude optimization |
| `gifski` | High-quality GIF encoding |
| `gif` | Low-level GIF encoding |

### OCR
| Crate | Purpose |
|-------|---------|
| `uni-ocr` | Cross-platform unified OCR (Vision on macOS, native on Windows) |
| `tesseract-rs` | Tesseract bindings (fallback) |

### Tauri Plugins
| Plugin | Purpose |
|--------|---------|
| `tauri-plugin-global-shortcut` | Global hotkeys |
| `tauri-plugin-system-tray` | Menu bar / system tray |
| `tauri-plugin-clipboard-manager` | Clipboard (text + images) |
| `tauri-plugin-screenshots` | Basic screenshots (xcap-based) |

---

## 8. Conclusion

Tauri v2 with Rust is a **strong alternative to Swift/SwiftUI** for Gaze. The existence of Cap (a production Tauri v2 screen recorder) removes the biggest risk -- proving the architecture works. The main trade-off is slightly higher memory usage vs gaining cross-platform support and a richer web UI ecosystem.

**If Gaze is macOS-only forever**: Swift/SwiftUI is still the most native option.
**If cross-platform is desired (even eventually)**: Tauri v2 is the better starting point. Retrofitting cross-platform support onto a Swift app is effectively impossible, while Tauri provides it from day one.
