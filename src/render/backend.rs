//! The GPU context Skia draws with, when the platform offers one.

use super::*;

#[cfg(target_os = "macos")]
pub(super) fn metal_context() -> Option<sk::gpu::DirectContext> {
    use objc2::rc::Retained;
    use objc2_metal::MTLDevice;
    if std::env::var("SUBTAKE_RENDERER").as_deref() == Ok("software") {
        return None;
    }
    let device = objc2_metal::MTLCreateSystemDefaultDevice()?;
    let queue = device.newCommandQueue()?;
    // SAFETY: both Objective-C objects are live retained objects. Skia's backend
    // and direct context retain their own references before these locals drop.
    let backend = unsafe {
        sk::gpu::mtl::BackendContext::new(
            Retained::as_ptr(&device) as sk::gpu::mtl::Handle,
            Retained::as_ptr(&queue) as sk::gpu::mtl::Handle,
        )
    };
    sk::gpu::direct_contexts::make_metal(&backend, None)
}
