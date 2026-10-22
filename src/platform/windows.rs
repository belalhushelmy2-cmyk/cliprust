use super::Target;
use anyhow::{Result, bail};
use windows_sys::Win32::{
    Foundation::CloseHandle,
    System::DataExchange::GetClipboardOwner,
    System::Threading::{
        GetCurrentProcessId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    },
    UI::{
        Input::KeyboardAndMouse::*,
        WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow},
    },
};
pub fn target() -> Result<Target> {
    unsafe {
        let window = GetForegroundWindow();
        let mut process = 0;
        GetWindowThreadProcessId(window, &mut process);
        // Repeated shortcut presses must not replace the destination with ourselves.
        Ok(Target(if process == GetCurrentProcessId() {
            0
        } else {
            window as u64
        }))
    }
}
pub fn source_application() -> Option<String> {
    unsafe {
        let window = GetClipboardOwner();
        if window.is_null() {
            return None;
        }
        let mut process_id = 0;
        if GetWindowThreadProcessId(window, &mut process_id) == 0 || process_id == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if process.is_null() {
            return None;
        }
        let mut path = vec![0u16; 32768];
        let mut length = path.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length);
        CloseHandle(process);
        if ok == 0 || length == 0 || length as usize > path.len() {
            return None;
        }
        let executable = String::from_utf16_lossy(&path[..length as usize]);
        let name = executable.rsplit(['\\', '/']).next()?;
        super::normalized_source_application(name)
    }
}
pub fn paste(target: Target) -> Result<()> {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            IsIconic, IsWindow, SW_RESTORE, ShowWindow,
        };
        if IsWindow(target.0 as _) == 0 {
            bail!("Previous application window was closed");
        }
        let released = || {
            [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN, VK_RETURN]
                .iter()
                .all(|key| GetAsyncKeyState(*key as i32) as u16 & 0x8000 == 0)
        };
        for _ in 0..100 {
            if released() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !released() {
            bail!("Release the shortcut keys and try Paste again");
        }
        if IsIconic(target.0 as _) != 0 {
            ShowWindow(target.0 as _, SW_RESTORE);
        }
        if SetForegroundWindow(target.0 as _) == 0 {
            bail!("Windows refused focus restoration");
        }
        if GetForegroundWindow() as u64 != target.0 {
            bail!("previous application did not receive focus");
        }
        let key = |vk: u16, flags: u32| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let keys = [
            key(VK_CONTROL, 0),
            key(0x56, 0),
            key(0x56, KEYEVENTF_KEYUP),
            key(VK_CONTROL, KEYEVENTF_KEYUP),
        ];
        if SendInput(
            keys.len() as u32,
            keys.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        ) != keys.len() as u32
        {
            bail!("Windows input was blocked (possibly an elevated target)");
        }
    }
    Ok(())
}

pub fn open_external(value: &str) -> Result<()> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    if value.is_empty() || value.contains('\0') {
        bail!("Invalid URL or file reference")
    }
    let verb: Vec<u16> = OsStr::new("open").encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = OsStr::new(value).encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        bail!("Windows could not open this URL or file reference")
    }
    Ok(())
}

pub fn hotkey(
    tx: crossbeam_channel::Sender<super::Event>,
    shortcut: crate::shortcuts::Shortcut,
) -> Result<()> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
    unsafe {
        if RegisterHotKey(
            std::ptr::null_mut(),
            1,
            (if shortcut.alt { MOD_ALT } else { 0 })
                | (if shortcut.shift { MOD_SHIFT } else { 0 })
                | (if shortcut.ctrl { MOD_CONTROL } else { 0 })
                | (if shortcut.meta { MOD_WIN } else { 0 })
                | MOD_NOREPEAT,
            shortcut.key.as_bytes()[0].to_ascii_uppercase() as u32,
        ) == 0
        {
            bail!("Configured global shortcut is unavailable or conflicts with another shortcut");
        }
        let mut message: MSG = std::mem::zeroed();
        loop {
            let result = GetMessageW(&mut message, std::ptr::null_mut(), 0, 0);
            if result == -1 {
                bail!("Windows hotkey event loop failed");
            }
            if result == 0 {
                break;
            }
            if message.message == WM_HOTKEY {
                let _ = tx.send(super::Event::Shortcut(target()?));
            }
        }
        UnregisterHotKey(std::ptr::null_mut(), 1);
    }
    Ok(())
}
