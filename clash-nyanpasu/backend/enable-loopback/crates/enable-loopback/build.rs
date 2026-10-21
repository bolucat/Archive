//! Generate a multi-resolution icon and embed the executable's sole application manifest.

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.svg");
    println!("cargo:rerun-if-changed=assets/icon-16.svg");
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"));
    let mut directory = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16, 20, 24, 32, 40, 48, 64, 256] {
        let asset = if size == 16 {
            "assets/icon-16.svg"
        } else {
            "assets/icon.svg"
        };
        let tree = resvg::usvg::Tree::from_data(
            &fs::read(asset).expect("read SVG icon"),
            &resvg::usvg::Options::default(),
        )
        .expect("parse SVG icon");
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("allocate icon bitmap");
        let scale = size as f32 / tree.size().width();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        // tiny-skia stores premultiplied pixels; PNG encoding restores straight alpha.
        let png = pixmap.encode_png().expect("encode icon PNG");
        let image = ico::IconImage::read_png(png.as_slice()).expect("decode icon PNG");
        directory.add_entry(ico::IconDirEntry::encode_as_png(&image).expect("pack icon PNG"));
    }
    let icon = out.join("enable-loopback.ico");
    directory
        .write(fs::File::create(&icon).expect("create ICO"))
        .expect("write ICO");
    let manifest = out.join("enable-loopback.manifest");
    fs::write(&manifest, r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="libnyanpasu.enable-loopback" type="win32"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges>
    <requestedExecutionLevel level="highestAvailable" uiAccess="false"/>
  </requestedPrivileges></security></trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1"><application>
    <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
  </application></compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings>
    <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    <heapType xmlns="http://schemas.microsoft.com/SMI/2020/WindowsSettings">SegmentHeap</heapType>
  </windowsSettings></application>
  <dependency><dependentAssembly>
    <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
  </dependentAssembly></dependency>
</assembly>
"#).expect("write application manifest");
    let rc = out.join("enable-loopback.rc");
    fs::write(
        &rc,
        format!(
            "1 ICON \"{}\"\n1 24 \"{}\"\n",
            icon.display().to_string().replace('\\', "/"),
            manifest.display().to_string().replace('\\', "/")
        ),
    )
    .expect("write resource script");
    // reactor a29e064 has no build.rs/linker manifest. Its self-contained check
    // probes RT_MANIFEST for a marker we omit, retaining framework-dependent bootstrap.
    // Embed RT_MANIFEST ourselves; suppress link.exe's generated default manifest.
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bin=enableLoopback=/MANIFEST:NO");
    }
    embed_resource::compile_for(&rc, ["enableLoopback"], embed_resource::NONE)
        .manifest_required()
        .expect("compile executable resources");
}
