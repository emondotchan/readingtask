use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
  let manifest_dir =
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
  let bundled_resources_dir = manifest_dir.join("resources");
  let bundled_db_path = bundled_resources_dir.join("bundled.reading.sqlite");
  let source_db_candidates = [
    env::var_os("HOME").map(PathBuf::from),
    env::var_os("USERPROFILE").map(PathBuf::from),
  ];

  for candidate in source_db_candidates.iter().flatten() {
    println!(
      "cargo:rerun-if-changed={}",
      candidate.join(".reading.sqlite").display()
    );
  }
  println!("cargo:rerun-if-env-changed=HOME");
  println!("cargo:rerun-if-env-changed=USERPROFILE");

  fs::create_dir_all(&bundled_resources_dir).expect("create bundled resources dir");

  let source_db_path = source_db_candidates
    .iter()
    .flatten()
    .map(|dir| dir.join(".reading.sqlite"))
    .find(|path| path.is_file());

  if let Some(source_db_path) = source_db_path {
    fs::copy(&source_db_path, &bundled_db_path).expect("copy sqlite template database");
  } else if !bundled_db_path.is_file() {
    panic!(
      "missing sqlite template database; checked HOME/USERPROFILE and {}",
      bundled_db_path.display()
    );
  }

  tauri_build::build();
}
