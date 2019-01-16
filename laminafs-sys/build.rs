extern crate bindgen;
extern crate cc;

use bindgen::builder;

fn main() {
	cc::Build::new()
		.cpp(true)
		.include("vendor/laminaFS/src/")
		.file("vendor/laminaFS/src/FileContext.cpp")
		.file("vendor/laminaFS/src/laminaFS_c.cpp")
		.file("vendor/laminaFS/src/device/Directory.cpp")
		.compile("laminafs");

	let bindings = builder()
		.header("vendor/laminaFS/src/laminaFS_c.h")
		.opaque_type("lfs_file_context_s")
		.opaque_type("lfs_file_context_t")
		.opaque_type("lfs_work_item_t")
		.opaque_type("void")
		.generate().unwrap();

	bindings.write_to_file("src/lib.rs");
}
