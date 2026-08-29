use std::path::{Component, Path as StdPath};

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use include_dir::{Dir, include_dir};

static WEB_DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/web/dist");

pub async fn serve_embedded_static(uri: Uri) -> Response {
  let request_path = uri.path().trim_start_matches('/');
  let is_root_or_html = request_path.is_empty() || request_path.ends_with(".html");
  let asset_path = if request_path.is_empty() {
    "index.html"
  } else {
    request_path
  };

  match embedded_asset(asset_path).or_else(|| embedded_asset("index.html")) {
    Some((path, contents)) => {
      let content_type = mime_guess::from_path(path).first_or_octet_stream();
      let path_str = path.to_string_lossy();
      let is_hashed_asset = path_str.starts_with("assets/")
        || path.extension().is_some_and(|ext| {
          ext == "js"
            || ext == "css"
            || ext == "woff2"
            || ext == "woff"
            || ext == "png"
            || ext == "svg"
            || ext == "webp"
        });

      let cache_control = if is_root_or_html || path_str.ends_with("index.html") {
        "no-cache"
      } else if is_hashed_asset {
        "public, max-age=31536000, immutable"
      } else {
        "public, max-age=86400"
      };

      (
        [
          (header::CONTENT_TYPE, content_type.as_ref()),
          (header::CACHE_CONTROL, cache_control),
        ],
        contents,
      )
        .into_response()
    }
    None => StatusCode::NOT_FOUND.into_response(),
  }
}

fn embedded_asset(path: &str) -> Option<(&'static StdPath, &'static [u8])> {
  let clean_path = sanitize_embedded_path(path)?;
  let file = WEB_DIST.get_file(clean_path)?;
  Some((file.path(), file.contents()))
}

fn sanitize_embedded_path(path: &str) -> Option<&str> {
  let path = path.trim_start_matches('/');
  if path.is_empty()
    || StdPath::new(path)
      .components()
      .any(|component| !matches!(component, Component::Normal(_)))
  {
    return None;
  }
  Some(path)
}
