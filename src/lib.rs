//! Pulse (脉搏) feed logic. `App.tsx` imports it from `theysay/native`.

mod bridge;
mod icloud;
mod legacy;
mod logic;
mod panels;
mod speech;
mod store;
mod translate;
mod notify;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod notify_mac;
#[cfg(target_os = "ios")]
mod refresh_ios;

use nsh_core::prelude::*;

/// Registers [`bridge::Core`] for the TSX screen.
#[native_module("theysay/native")]
fn native(registry: &mut Registry) {
    registry.module::<bridge::Core>();
    // iOS only. Must run during launch, before the system asks for handlers.
    #[cfg(target_os = "ios")]
    crate::refresh_ios::install();
}
