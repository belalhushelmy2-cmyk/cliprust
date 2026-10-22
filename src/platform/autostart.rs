use anyhow::{Context, Result, bail};

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use std::{
        fs::{self, OpenOptions},
        io::Write,
        os::unix::fs::OpenOptionsExt,
        path::{Path, PathBuf},
    };

    fn desktop_quote(value: &str) -> Result<String> {
        if value.chars().any(char::is_control) {
            bail!("Executable path contains control characters")
        }
        let mut quoted = String::from("\"");
        for ch in value.chars() {
            if matches!(ch, '\\' | '"' | '`' | '$') {
                quoted.push('\\');
            }
            quoted.push(ch);
        }
        quoted.push('"');
        Ok(quoted)
    }

    fn file_path(config_dir: &Path) -> PathBuf {
        config_dir.join("autostart").join("cliprust.desktop")
    }

    fn set_at(config_dir: &Path, executable: &Path, enabled: bool) -> Result<()> {
        let path = file_path(config_dir);
        if !enabled {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("remove autostart entry"),
            }
            return Ok(());
        }

        let parent = path.parent().context("autostart directory has no parent")?;
        fs::create_dir_all(parent).context("create XDG autostart directory")?;
        let command = format!("{} --show", desktop_quote(&executable.to_string_lossy())?);
        let body = format!(
            "[Desktop Entry]\nType=Application\nName=Cliprust\nComment=Clipboard history\nExec={command}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
        );
        let temporary = parent.join(format!(".cliprust-{}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&temporary)
            .context("create temporary autostart entry")?;
        if let Err(error) = (|| -> std::io::Result<()> {
            file.write_all(body.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, &path)
        })() {
            let _ = fs::remove_file(&temporary);
            return Err(error).context("write XDG autostart entry");
        }
        Ok(())
    }

    pub fn enabled() -> Result<bool> {
        let config_dir = directories::BaseDirs::new()
            .context("user configuration directory unavailable")?
            .config_dir()
            .to_owned();
        Ok(file_path(&config_dir).is_file())
    }

    pub fn set(enabled: bool) -> Result<()> {
        let config_dir = directories::BaseDirs::new()
            .context("user configuration directory unavailable")?
            .config_dir()
            .to_owned();
        let executable = super::super::launcher_path().context("resolve Cliprust launcher")?;
        set_at(&config_dir, &executable, enabled)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn autostart_entry_is_written_quoted_and_removed_idempotently() {
            let temp = tempfile::tempdir().unwrap();
            let executable = temp.path().join("Cliprust build/cliprust");
            set_at(temp.path(), &executable, true).unwrap();
            let path = file_path(temp.path());
            let body = fs::read_to_string(&path).unwrap();
            assert!(body.contains("Exec=\""));
            assert!(body.contains("Cliprust build/cliprust\" --show"));
            set_at(temp.path(), &executable, false).unwrap();
            set_at(temp.path(), &executable, false).unwrap();
            assert!(!path.exists());
        }

        #[test]
        fn autostart_rejects_control_characters_in_executable_path() {
            assert!(desktop_quote("cliprust\nmalicious").is_err());
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS},
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey,
            RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
        },
    };

    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const VALUE_NAME: &str = "Cliprust";

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn quote_windows_argument(value: &std::ffi::OsStr) -> String {
        let mut out = String::from("\"");
        let mut slashes = 0;
        for ch in value.to_string_lossy().chars() {
            match ch {
                '\\' => slashes += 1,
                '"' => {
                    out.push_str(&"\\".repeat(slashes * 2 + 1));
                    out.push('"');
                    slashes = 0;
                }
                _ => {
                    out.push_str(&"\\".repeat(slashes));
                    out.push(ch);
                    slashes = 0;
                }
            }
        }
        out.push_str(&"\\".repeat(slashes * 2));
        out.push('"');
        out
    }

    pub fn enabled() -> Result<bool> {
        let key_path = wide(RUN_KEY);
        let value_name = wide(VALUE_NAME);
        let mut key: HKEY = std::ptr::null_mut();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                key_path.as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut key,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        if status != ERROR_SUCCESS {
            bail!("open current-user startup registry key failed: {status}")
        }
        let mut data_size = 0u32;
        let status = unsafe {
            RegQueryValueExW(
                key,
                value_name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut data_size,
            )
        };
        unsafe { RegCloseKey(key) };
        match status {
            ERROR_SUCCESS | ERROR_MORE_DATA => Ok(true),
            ERROR_FILE_NOT_FOUND => Ok(false),
            _ => bail!("query current-user startup registry value failed: {status}"),
        }
    }

    pub fn set(enabled: bool) -> Result<()> {
        let key_path = wide(RUN_KEY);
        let value_name = wide(VALUE_NAME);
        let command_data = if enabled {
            let executable = super::super::launcher_path().context("resolve Cliprust launcher")?;
            let command = format!("{} --show", quote_windows_argument(executable.as_os_str()));
            Some(wide(&command))
        } else {
            None
        };
        let mut key: HKEY = std::ptr::null_mut();
        let status = unsafe {
            if enabled {
                RegCreateKeyW(HKEY_CURRENT_USER, key_path.as_ptr(), &mut key)
            } else {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    key_path.as_ptr(),
                    0,
                    KEY_SET_VALUE,
                    &mut key,
                )
            }
        };
        if status == ERROR_FILE_NOT_FOUND && !enabled {
            return Ok(());
        }
        if status != ERROR_SUCCESS {
            bail!("open current-user startup registry key failed: {status}")
        }
        let status = if enabled {
            let data = command_data
                .as_ref()
                .context("startup command was not prepared")?;
            unsafe {
                RegSetValueExW(
                    key,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    data.as_ptr().cast(),
                    std::mem::size_of_val(data.as_slice()) as u32,
                )
            }
        } else {
            unsafe { RegDeleteValueW(key, value_name.as_ptr()) }
        };
        unsafe { RegCloseKey(key) };
        if status == ERROR_FILE_NOT_FOUND && !enabled {
            return Ok(());
        }
        if status != ERROR_SUCCESS {
            bail!("update current-user startup registry value failed: {status}")
        }
        Ok(())
    }
}

pub fn enabled() -> Result<bool> {
    #[cfg(any(target_os = "linux", windows))]
    {
        imp::enabled()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        bail!("launch at login is unsupported on this platform")
    }
}

pub fn set(enabled: bool) -> Result<()> {
    #[cfg(any(target_os = "linux", windows))]
    {
        imp::set(enabled)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = enabled;
        bail!("launch at login is unsupported on this platform")
    }
}
