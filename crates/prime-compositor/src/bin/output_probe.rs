use prime_compositor::output::initialize_primary_output;
use smithay::{
    backend::{
        allocator::gbm::GbmDevice,
        drm::DrmDeviceFd,
        egl::{context::ContextPriority, EGLContext, EGLDisplay},
        renderer::gles::GlesRenderer,
        session::{libseat::LibSeatSession, Session},
        udev::primary_gpu,
    },
    reexports::rustix::fs::OFlags,
    utils::DeviceFd,
};
use std::{error::Error, io};

fn main() -> Result<(), Box<dyn Error>> {
    let (mut session, _notifier) = LibSeatSession::new()?;
    let seat_name = session.seat();
    if !session.is_active() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("libseat session {seat_name} is inactive"),
        )
        .into());
    }

    let primary_gpu_path = primary_gpu(&seat_name)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no primary DRM GPU is available on seat {seat_name}"),
        )
    })?;
    let fd = session.open(
        &primary_gpu_path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
    )?;
    let drm_fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let gbm = GbmDevice::new(drm_fd.clone())?;
    let egl_display = unsafe { EGLDisplay::new(gbm.clone())? };
    let egl_context = EGLContext::new_with_priority(&egl_display, ContextPriority::High)?;
    let mut renderer = unsafe { GlesRenderer::new(egl_context)? };

    let backend = initialize_primary_output(drm_fd, gbm, &mut renderer)?;
    println!(
        "PRIME_OUTPUT_PROBE=READY connector={} mode={}x{} refresh_millihz={}",
        backend.connector_name,
        backend.mode.size.w,
        backend.mode.size.h,
        backend.mode.refresh
    );
    Ok(())
}
