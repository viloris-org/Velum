use std::sync::{Mutex, OnceLock};

use tokio::sync::watch;

use crate::{executor, handles::handles};

fn active_tun() -> &'static Mutex<Option<watch::Sender<bool>>> {
    static ACTIVE: OnceLock<Mutex<Option<watch::Sender<bool>>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(None))
}

/// Runs the Android TUN engine for one online runtime handle.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_android_tun_run(runtime_handle: u64, tun_fd: i32) -> i32 {
    velum_client_android_tun_run_v2(runtime_handle, tun_fd, 1500)
}

/// Runs the Android TUN engine with the validated platform MTU.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_android_tun_run_v2(
    runtime_handle: u64,
    tun_fd: i32,
    mtu: u16,
) -> i32 {
    if tun_fd < 0 {
        return -1;
    }
    if mtu < 576 {
        return -1;
    }
    let entry = match handles().lock() {
        Ok(table) => table.clients.get(&runtime_handle).cloned(),
        Err(_) => return -2,
    };
    let Some(entry) = entry else {
        return -1;
    };
    let (shutdown, stopped) = watch::channel(false);
    match active_tun().lock() {
        Ok(mut active) if active.is_none() => *active = Some(shutdown),
        Ok(_) => return -3,
        Err(_) => return -2,
    }
    let result = executor().block_on(velum_adapter_tun::run_android_tun(
        entry.runtime.clone(),
        tun_fd,
        mtu,
        stopped,
    ));
    if let Ok(mut active) = active_tun().lock() {
        active.take();
    }
    if result.is_ok() { 0 } else { -4 }
}

/// Cancels the active Android packet engine. It is idempotent.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_android_tun_stop() -> i32 {
    match active_tun().lock() {
        Ok(active) => {
            if let Some(shutdown) = active.as_ref() {
                shutdown.send_replace(true);
            }
            0
        }
        Err(_) => -1,
    }
}

/// Android service-owned emergency stop path used after revoke or destruction.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_velum_velum_1client_NativeTun_stop(
    _environment: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
) -> i32 {
    velum_client_android_tun_stop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_descriptor_without_installing_engine() {
        assert_eq!(velum_client_android_tun_run(u64::MAX, -1), -1);
        assert_eq!(velum_client_android_tun_run_v2(u64::MAX, 1, 500), -1);
        assert!(active_tun().lock().expect("state").is_none());
    }
}
