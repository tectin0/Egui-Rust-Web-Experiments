#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod web;

pub const HOST: &str = "http://127.0.0.1:8000";
