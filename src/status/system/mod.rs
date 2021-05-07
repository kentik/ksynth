pub use unix::system;

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod unix;

#[cfg(not(target_os = "linux"))]
#[path = "posix.rs"]
mod unix;
