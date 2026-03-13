use std::path::PathBuf;

use include_dir::{include_dir, Dir};

use crate::errors::Error;

// Embed only the Max for Live device files, not the entire repo
static ABLETON_OSC_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/AbletonOSC/AbletonOSC");

/// Returns the platform-specific Ableton User Library path for Max MIDI Effects.
fn ableton_midi_effects_path() -> Result<PathBuf, Error> {
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

/// Install AbletonOSC into the Ableton User Library.
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

    // Extract embedded files to target directory
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
