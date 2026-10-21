// SPDX-License-Identifier: GPL-3.0-or-later
//! Cooperative maintenance shutdown, scoped to this user's data directory.
use std::{
    fs::OpenOptions,
    path::Path,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
    System::Threading::{
        CreateEventW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
    },
};
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn name(directory: &Path) -> Vec<u16> {
    format!(
        "Local\\MeltypeRust-Maintenance-{:032x}",
        crate::windows_services::tray_guid(directory)
    )
    .encode_utf16()
    .chain(Some(0))
    .collect()
}
pub struct StopEvent(Handle);
impl StopEvent {
    /// The caller holds .tray.lock before publishing the event.
    pub fn new(directory: &Path) -> Result<Self, String> {
        let event = unsafe { CreateEventW(std::ptr::null(), 0, 0, name(directory).as_ptr()) };
        if event.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(Self(Handle(event)))
    }
    pub fn requested(&self) -> bool {
        unsafe { WaitForSingleObject(self.0 .0, 0) == WAIT_OBJECT_0 }
    }
}
/// Wait for normal tray teardown. Never kills a process or changes Enabled.
pub fn stop(directory: &Path) -> Result<bool, String> {
    let lock = match OpenOptions::new()
        .read(true)
        .write(true)
        .open(directory.join(".tray.lock"))
    {
        Ok(lock) => lock,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    if fs2::FileExt::try_lock_exclusive(&lock).is_ok() {
        return Ok(false);
    }
    let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name(directory).as_ptr()) };
    if event.is_null() {
        return Err("稼働中のトレイは自動終了に対応していません。手動で終了してください。".into());
    }
    let event = Handle(event);
    if unsafe { SetEvent(event.0) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if fs2::FileExt::try_lock_exclusive(&lock).is_ok() {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(
        "トレイを終了できませんでした。未保存の設定を保存または取り消し、手動で終了してください。"
            .into(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_tray_does_not_create_data() {
        let temp = tempfile::tempdir().unwrap();
        let absent = temp.path().join("missing");
        assert_eq!(stop(&absent), Ok(false));
        assert!(!absent.exists());
    }
    #[test]
    fn cooperative_signal_is_scoped_and_waits_for_lock_release() {
        let temp = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(temp.path().join(".tray.lock"))
            .unwrap();
        fs2::FileExt::lock_exclusive(&file).unwrap();
        let event = StopEvent::new(temp.path()).unwrap();
        let unrelated = StopEvent::new(other.path()).unwrap();
        let directory = temp.path().to_owned();
        let worker = std::thread::spawn(move || stop(&directory));
        let deadline = Instant::now() + Duration::from_secs(3);
        while !event.requested() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!unrelated.requested());
        assert!(
            !worker.is_finished(),
            "signal must not imply completed shutdown"
        );
        drop(file);
        assert_eq!(worker.join().unwrap(), Ok(true));
        assert_eq!(stop(temp.path()), Ok(false));
    }
}
