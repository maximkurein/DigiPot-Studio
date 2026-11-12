# Bundling the egui binary

Tauri's bundler only packages Tauri apps, so the egui binary is bundled by hand. In DigiPot this is
a `--egui` flag of `build_macos.py`; `assets/template/build_egui_macos.py` is a standalone version.

## macOS .app

```text
<Product> egui.app/Contents/
  Info.plist
  PkgInfo                 "APPL????"
  MacOS/<app>-egui
  Resources/icon.icns     reuse src-tauri/icons/icon.icns
```

Info.plist keys: `CFBundleName`, `CFBundleDisplayName`, `CFBundleExecutable`,
`CFBundleIdentifier` (**different** from the Tauri identifier, e.g. `com.example.egui` vs
`com.example.desktop` — otherwise LaunchServices treats them as one app),
`CFBundleIconFile`, `CFBundlePackageType=APPL`, `CFBundleShortVersionString` and
`CFBundleVersion` (from the project's single version source), `LSMinimumSystemVersion`,
`NSHighResolutionCapable=true`, `NSHumanReadableCopyright`, `LSApplicationCategoryType`.
Validate with `plutil -lint`.

Steps:

1. version check (refuse on drift), `cargo test` core + egui;
2. `cargo build --release -p <app>-egui`; universal: build `aarch64-apple-darwin` and
   `x86_64-apple-darwin`, then `lipo -create -output <bin> <arm> <x86>`;
3. assemble the bundle in `target/egui-bundle/dmg/`;
4. `codesign --force --deep --sign - --identifier <bundle id> <app>` and
   `codesign --verify --strict <app>` (ad-hoc; Apple Silicon refuses unsigned apps as "damaged");
5. `.app.zip`: `ditto -c -k --sequesterRsrc --keepParent <app> <zip>`;
6. `.dmg`: add `Applications -> /Applications` symlink next to the app, then
   `hdiutil create -volname "<Product> <v>" -srcfolder <staging> -ov -format UDZO <dmg>` and
   `hdiutil verify <dmg>`;
7. merge SHA-256 lines into `release/vX.Y.Z/checksum.txt`.

Check: `hdiutil attach -nobrowse -readonly -mountpoint <tmp> <dmg>`, `codesign -dv`,
`lipo -archs`, `open -n <app>`, `pgrep -lf MacOS/<app>-egui`, then detach.

Notarization: replace `-` with a Developer ID identity, add `--options runtime --timestamp`, then
`xcrun notarytool submit --wait` and `xcrun stapler staple`.

## Windows

The binary already hides the console with
`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`. Embed the icon with the
`winres`/`embed-resource` build dependency (`icon.ico` from `src-tauri/icons`), then ship a zip or
an Inno Setup / WiX installer. A separate AppId/UpgradeCode from the Tauri installer.

## Linux

`cargo build --release`, plus a `.desktop` file and the PNG icon; AppImage via `cargo-appimage`
or `linuxdeploy` if needed.
