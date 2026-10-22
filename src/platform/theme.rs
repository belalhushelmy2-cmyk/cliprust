use super::Event;
#[cfg(target_os = "linux")]
use anyhow::Context;
use anyhow::Result;
use crossbeam_channel::Sender as SyncSender;

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use ashpd::desktop::settings::Settings;
    use futures_util::StreamExt;

    pub fn system_uses_light() -> Result<bool> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("start desktop appearance portal runtime")?;
        runtime.block_on(async {
            let settings = Settings::new()
                .await
                .context("connect to the desktop appearance portal")?;
            let scheme = settings
                .color_scheme()
                .await
                .context("read system color scheme")?;
            Ok(matches!(
                scheme,
                ashpd::desktop::settings::ColorScheme::PreferLight
            ))
        })
    }

    pub fn watch(tx: SyncSender<Event>) -> Result<()> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("start desktop appearance watcher")?;
        runtime.block_on(async move {
            let settings = Settings::new()
                .await
                .context("connect to the desktop appearance portal")?;
            let initial = settings
                .color_scheme()
                .await
                .context("read system color scheme")?;
            tx.send(Event::SystemTheme(matches!(
                initial,
                ashpd::desktop::settings::ColorScheme::PreferLight
            )))
            .map_err(|_| anyhow::anyhow!("theme event receiver stopped"))?;

            let mut changed = settings
                .receive_setting_changed()
                .await
                .context("subscribe to system appearance changes")?;
            while let Some(setting) = changed.next().await {
                if setting.namespace() == ashpd::desktop::settings::APPEARANCE_NAMESPACE
                    && setting.key() == ashpd::desktop::settings::COLOR_SCHEME_KEY
                {
                    let scheme =
                        ashpd::desktop::settings::ColorScheme::try_from(setting.value().clone())
                            .context("decode system color scheme change")?;
                    tx.send(Event::SystemTheme(matches!(
                        scheme,
                        ashpd::desktop::settings::ColorScheme::PreferLight
                    )))
                    .map_err(|_| anyhow::anyhow!("theme event receiver stopped"))?;
                }
            }
            Ok(())
        })
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, KEY_QUERY_VALUE, REG_DWORD,
            REG_NOTIFY_CHANGE_LAST_SET, RegCloseKey, RegNotifyChangeKeyValue, RegOpenKeyExW,
            RegQueryValueExW,
        },
    };

    const THEME_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
    const LIGHT_VALUE: &str = "AppsUseLightTheme";

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn system_uses_light() -> Result<bool> {
        let key_path = wide(THEME_KEY);
        let value_name = wide(LIGHT_VALUE);
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
            anyhow::bail!("open Windows appearance registry key failed: {status}")
        }
        let mut value_type = 0u32;
        let mut value = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            RegQueryValueExW(
                key,
                value_name.as_ptr(),
                std::ptr::null_mut(),
                &mut value_type,
                (&mut value as *mut u32).cast(),
                &mut size,
            )
        };
        unsafe { RegCloseKey(key) };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        if status != ERROR_SUCCESS {
            anyhow::bail!("read Windows appearance registry value failed: {status}")
        }
        Ok(value_type == REG_DWORD && size >= 4 && value != 0)
    }

    pub fn watch(tx: SyncSender<Event>) -> Result<()> {
        let key_path = wide(THEME_KEY);
        let mut key: HKEY = std::ptr::null_mut();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                key_path.as_ptr(),
                0,
                KEY_QUERY_VALUE | KEY_NOTIFY,
                &mut key,
            )
        };
        if status != ERROR_SUCCESS {
            anyhow::bail!("open Windows appearance registry watcher failed: {status}")
        }
        loop {
            let status = unsafe {
                RegNotifyChangeKeyValue(key, 0, REG_NOTIFY_CHANGE_LAST_SET, std::ptr::null_mut(), 0)
            };
            if status != ERROR_SUCCESS {
                unsafe { RegCloseKey(key) };
                anyhow::bail!("watch Windows appearance registry failed: {status}")
            }
            let mut value_type = 0u32;
            let mut value = 0u32;
            let mut size = std::mem::size_of::<u32>() as u32;
            let status = unsafe {
                RegQueryValueExW(
                    key,
                    wide(LIGHT_VALUE).as_ptr(),
                    std::ptr::null_mut(),
                    &mut value_type,
                    (&mut value as *mut u32).cast(),
                    &mut size,
                )
            };
            if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
                unsafe { RegCloseKey(key) };
                anyhow::bail!("read Windows appearance registry change failed: {status}")
            }
            let light =
                status == ERROR_SUCCESS && value_type == REG_DWORD && size >= 4 && value != 0;
            if tx.send(Event::SystemTheme(light)).is_err() {
                unsafe { RegCloseKey(key) };
                return Err(anyhow::anyhow!("theme event receiver stopped"));
            }
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod imp {
    use super::*;

    pub fn system_uses_light() -> Result<bool> {
        Ok(false)
    }

    pub fn watch(tx: SyncSender<Event>) -> Result<()> {
        tx.send(Event::SystemTheme(false))
            .map_err(|_| anyhow::anyhow!("theme event receiver stopped"))
    }
}

pub fn system_uses_light() -> Result<bool> {
    imp::system_uses_light()
}

pub fn watch(tx: SyncSender<Event>) -> Result<()> {
    imp::watch(tx)
}
