extern crate bindgen;
extern crate cc;

use bindgen::builder;

fn main() {
	cc::Build::new()
		.cpp(true)
		.include("../../src/")
		.file("../../src/FileContext.cpp")
		.file("../../src/laminaFS_c.cpp")
		.file("../../src/device/Directory.cpp")
		.compile("laminafs");

	let bindings = builder().header("../../src/laminaFS_c.h").generate().unwrap();
		
	bindings.write_to_file("src/lib.rs");
}
