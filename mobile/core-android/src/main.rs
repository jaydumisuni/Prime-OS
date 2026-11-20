//! Android native init-managed read-only Prime platform adapter.
//!
//! On Android the only accepted listener comes from the FD supplied by
//! Android init's socket directive, not a public TCP port, arbitrary
//! filesystem socket, or auto-created app-service.
//! This is NOT production bootc-qualified Core; it cannot mutate devices.
use prime_mobile_core_android::AndroidCoreSnapshot;
use std::env;
use std::io::{self, Read, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;

const MAX_REQUEST: usize = 8192;

#[cfg(target_os = "android")]
fn listener() -> io::Result<UnixListener> {
    let raw = env::var("ANDROID_SOCKET_prime_mobile_core")
        .map_err(|_| io::Error::new(io::ErrorKind::PermissionDenied, "INIT_SOCKET_REQUIRED"))?;
    let fd: RawFd = raw.parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "INVALID_INIT_SOCKET_FD"))?;
    if fd < 3 { return Err(io::Error::new(io::ErrorKind::InvalidInput, "RESERVED_INIT_FD")); }
    let mut kind: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(fd, libc::SOL_SOCKET, libc::SO_TYPE,
                         (&mut kind as *mut libc::c_int).cast(), &mut size)
    };
    if rc != 0 || kind != libc::SOCK_STREAM {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "INIT_FD_NOT_STREAM_SOCKET"));
    }
    let mut accepting: libc::c_int = 0;
    let mut accepting_size = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(fd, libc::SOL_SOCKET, libc::SO_ACCEPTCONN,
                         (&mut accepting as *mut libc::c_int).cast(), &mut accepting_size)
    };
    if rc != 0 || accepting != 1 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "INIT_FD_NOT_LISTENER"));
    }
    // Android init owns socket path, mode and SELinux context. Do not unlink,
    // chmod, chown or create an alternative listener.
    Ok(unsafe { UnixListener::from_raw_fd(fd) })
}

#[cfg(not(target_os = "android"))]
fn listener() -> io::Result<UnixListener> {
    // Only a host-side fixture path; no target phone route and no default
    // listener. Caller must explicitly set this in an isolated test.
    let path = env::var("PRIME_MOBILE_TEST_SOCKET")
        .map_err(|_| io::Error::new(io::ErrorKind::PermissionDenied, "TEST_SOCKET_REQUIRED"))?;
    if !path.starts_with("/tmp/") || path.len() > 100 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "TEST_SOCKET_PATH_UNSAFE"));
    }
    UnixListener::bind(path)
}

fn handle_client(mut client: UnixStream, state: &AndroidCoreSnapshot) -> io::Result<()> {
    client.set_read_timeout(Some(Duration::from_millis(1500)))?;
    client.set_write_timeout(Some(Duration::from_millis(1500)))?;
    let mut request = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while request.len() <= MAX_REQUEST {
        let read = client.read(&mut chunk)?;
        if read == 0 { break; }
        request.extend_from_slice(&chunk[..read]);
        if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            break;
        }
    }
    let response = state.reply(&request);
    client.write_all(&response)?;
    Ok(())
}

fn main() -> io::Result<()> {
    let listener = listener()?;
    let state = AndroidCoreSnapshot::probe()
        .map_err(|_| io::Error::other("READ_ONLY_PRIME_PROBE_FAILED"))?;
    eprintln!("prime-mobile-android: read-only interface, device admission unqualified");
    for peer in listener.incoming() {
        match peer {
            Ok(client) => {
                if let Err(error) = handle_client(client, &state) {
                    // Never log raw request, peer identity or device secrets.
                    eprintln!("prime-mobile-android: dropped invalid read-only request: {}", error.kind());
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
