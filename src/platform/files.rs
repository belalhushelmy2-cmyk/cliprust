use anyhow::{Context, Result, bail};
use std::path::PathBuf;

/// Run on a dedicated dialog thread, never on the UI or capture worker.
pub fn choose(save: bool, name: &str) -> Result<Option<PathBuf>> {
    #[cfg(target_os = "linux")]
    {
        use ashpd::desktop::file_chooser::SelectedFiles;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        return runtime.block_on(async {
            let response = if save {
                SelectedFiles::save_file()
                    .title("Save Cliprust file")
                    .current_name(name)
                    .send()
                    .await?
                    .response()
            } else {
                SelectedFiles::open_file()
                    .title("Import Cliprust history")
                    .multiple(false)
                    .send()
                    .await?
                    .response()
            };
            let selected = match response {
                Ok(selected) => selected,
                Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
                    return Ok(None);
                }
                Err(error) => return Err(error.into()),
            };
            let uri = selected.uris().first().context("No file selected")?;
            let path = url::Url::parse(&uri.to_string())?
                .to_file_path()
                .map_err(|_| anyhow::anyhow!("Choose a local file"))?;
            Ok(Some(path))
        });
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Controls::Dialogs::*;
        let mut path = vec![0u16; 32768];
        let initial: Vec<_> = name.encode_utf16().collect();
        if initial.len() >= path.len() {
            bail!("Filename is too long");
        }
        path[..initial.len()].copy_from_slice(&initial);
        let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
        dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        dialog.lpstrFile = path.as_mut_ptr();
        dialog.nMaxFile = path.len() as u32;
        dialog.Flags = OFN_EXPLORER
            | OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST
            };
        let accepted = unsafe {
            if save {
                GetSaveFileNameW(&mut dialog)
            } else {
                GetOpenFileNameW(&mut dialog)
            }
        };
        if accepted == 0 {
            let error = unsafe { CommDlgExtendedError() };
            if error == 0 {
                return Ok(None);
            }
            bail!("Windows file dialog failed: {error}");
        }
        use std::os::windows::ffi::OsStringExt;
        let end = path
            .iter()
            .position(|c| *c == 0)
            .context("Invalid selected filename")?;
        return Ok(Some(std::ffi::OsString::from_wide(&path[..end]).into()));
    }
    #[allow(unreachable_code)]
    {
        let _ = (save, name);
        bail!("Native file dialogs are unavailable on this platform")
    }
}
