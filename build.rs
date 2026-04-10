use anyhow::*;
use fs_extra::copy_items;
use fs_extra::dir::CopyOptions;
use std::env;
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    // This tells cargo to rerun this script if something in res/ changes.
    println!("cargo:rerun-if-changed=res/*");

    // Only copy for release builds
    if env::var("PROFILE")? == "release" {
        // Prepare what to copy and how
        let mut copy_options = CopyOptions::new();
        copy_options.overwrite = true;
        let paths_to_copy = vec!["res/"];

        // Copy the items to the target/release directory next to the exe
        let target_release_dir = Path::new(&env::var("OUT_DIR")?)
            .parent().unwrap()  // build
            .parent().unwrap()  // target
            .parent().unwrap()  // release
            .join("res");
        fs::create_dir_all(&target_release_dir)?;
        copy_items(&paths_to_copy, &target_release_dir, &copy_options)?;
    }

    Ok(())
}