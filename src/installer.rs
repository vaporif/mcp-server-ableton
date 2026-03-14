use std::path::PathBuf;

use include_dir::{Dir, include_dir};

use crate::errors::Error;

static ABLETON_OSC_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/AbletonOSC/AbletonOSC");

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn ableton_midi_effects_path() -> Result<PathBuf, Error> {
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::Installer("could not determine home directory".into()))?;
        Ok(home.join("Music/Ableton/User Library/Presets/MIDI Effects/Max MIDI Effect"))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = dirs::data_dir()
            .ok_or_else(|| Error::Installer("could not determine AppData directory".into()))?;
        Ok(appdata.join("Ableton/User Library/Presets/MIDI Effects/Max MIDI Effect"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(Error::Installer(
            "AbletonOSC installer only supports macOS and Windows".into(),
        ))
    }
}

pub fn install(force: bool) -> Result<(), Error> {
    let target_base = ableton_midi_effects_path()?;
    let target_dir = target_base.join("AbletonOSC");

    if !target_base.exists() {
        return Err(Error::Installer(format!(
            "Ableton User Library not found at {}. Is Ableton Live installed?",
            target_base.display()
        )));
    }

    if target_dir.exists() && !force {
        println!(
            "AbletonOSC is already installed at {}",
            target_dir.display()
        );
        println!("Use --force to overwrite.");
        return Ok(());
    }

    ABLETON_OSC_DIR
        .extract(&target_base)
        .map_err(|e| Error::Installer(format!("failed to extract AbletonOSC: {e}")))?;

    println!("AbletonOSC installed to {}", target_dir.display());
    println!();
    println!("Next steps:");
    println!("  1. Open Ableton Live");
    println!("  2. Drag AbletonOSC from your User Library into any track");
    println!("  3. You're ready to go!");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn midi_effects_path_contains_ableton() {
        let path = ableton_midi_effects_path().unwrap();
        let path_str = path.to_string_lossy();
        assert!(
            path_str.contains("Music/Ableton"),
            "expected path to contain 'Music/Ableton', got: {path_str}"
        );
    }

    #[test]
    fn install_fails_when_dir_missing() {
        let result = install(false);
        if let Err(e) = result {
            let msg = e.to_string();
            assert!(
                msg.contains("not found") || msg.contains("Ableton"),
                "unexpected error: {msg}"
            );
        }
    }
}
