use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
  let manifest_dir =
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
  let home_dir = PathBuf::from(env::var("HOME").expect("missing HOME environment variable"));
  let source_db_path = home_dir.join(".reading.sqlite");
  let bundled_resources_dir = manifest_dir.join("resources");
  let bundled_db_path = bundled_resources_dir.join("bundled.reading.sqlite");

  println!("cargo:rerun-if-changed={}", source_db_path.display());
  println!("cargo:rerun-if-env-changed=HOME");

  if !source_db_path.is_file() {
    panic!(
      "missing sqlite template database at {}",
      source_db_path.display()
    );
  }

  fs::create_dir_all(&bundled_resources_dir).expect("create bundled resources dir");
  fs::copy(&source_db_path, &bundled_db_path).expect("copy sqlite template database");

  tauri_build::build();
}
