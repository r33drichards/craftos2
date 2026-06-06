use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.parent().unwrap().to_path_buf();

    // Compile the C-ABI simulation shim against the emulator headers.
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file(repo.join("embed/ccsim.cpp"))
        .include(repo.join("src"))
        .include(repo.join("api"))
        .include(repo.join("craftos2-lua/include"))
        .compile("ccsim");

    // Link the prebuilt emulator archive (everything except main) + lua + deps.
    // libcraftos2.a is produced by embed/build.sh.
    println!("cargo:rustc-link-search=native={}", repo.join("embed").display());
    println!("cargo:rustc-link-lib=static=craftos2");

    let lua_dir = repo.join("craftos2-lua/src");
    println!("cargo:rustc-link-search=native={}", lua_dir.display());
    println!("cargo:rustc-link-lib=dylib=lua");
    // liblua.dylib has a relative install name; embed an rpath so it's found.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lua_dir.display());

    for lib in [
        "PocoNetSSL", "PocoNet", "PocoCrypto", "PocoJSON", "PocoXML", "PocoUtil",
        "PocoFoundation", "crypto", "ssl", "SDL2", "png", "c++",
    ] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
    println!("cargo:rustc-link-lib=framework=ApplicationServices");

    println!("cargo:rerun-if-changed={}", repo.join("embed/ccsim.cpp").display());
}
