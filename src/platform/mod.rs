pub mod process;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub mod macos_ax;
#[cfg(target_os = "macos")]
pub mod macos_legacy;
#[cfg(target_os = "macos")]
pub mod macos_cg;
#[cfg(target_os = "macos")]
pub mod macos_permissions;
#[cfg(target_os = "macos")]
pub mod macos_vision;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use linux as backend;
#[cfg(target_os = "macos")]
pub use macos as backend;
#[cfg(target_os = "windows")]
pub use windows as backend;
