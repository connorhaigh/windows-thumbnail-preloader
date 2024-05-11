fn main() {
	println!("cargo:rerun-if-changed=build.rs");

	#[cfg(target_os = "windows")]
	{
		use std::path::Path;

		if std::env::var("CARGO_CFG_TARGET_ENV").unwrap() == "msvc" {
			println!("cargo:rerun-if-changed=manifest.xml");

			println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
			println!(
				"cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
				Path::new("manifest.xml")
					.canonicalize()
					.expect("failed to find manifest file")
					.display()
			);
		}
	}
}
