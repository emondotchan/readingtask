use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
  let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
  let web_dir = Path::new(&manifest_dir).join("web");

  // Re-run the build script if any of these files/directories change
  println!("cargo:rerun-if-changed=web/src");
  println!("cargo:rerun-if-changed=web/index.html");
  println!("cargo:rerun-if-changed=web/package.json");
  println!("cargo:rerun-if-changed=web/package-lock.json");
  println!("cargo:rerun-if-changed=web/vite.config.ts");
  println!("cargo:rerun-if-changed=web/tsconfig.json");
  println!("cargo:rerun-if-changed=web/components.json");

  // Only build frontend if the web directory exists
  if web_dir.exists() {
    // Run `npm install`
    let npm_install_status = Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" })
      .current_dir(&web_dir)
      .args(["install"])
      .status()
      .expect("Failed to execute npm install");

    if !npm_install_status.success() {
      panic!("npm install failed with status: {}", npm_install_status);
    }

    // Run `npm run build`
    let npm_build_status = Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" })
      .current_dir(&web_dir)
      .args(["run", "build"])
      .status()
      .expect("Failed to execute npm run build");

    if !npm_build_status.success() {
      panic!("npm run build failed with status: {}", npm_build_status);
    }
  }
}
