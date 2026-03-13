use rosc::OscType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::errors::Error;
use crate::server::AbletonMcpServer;
use crate::tools::common::{self, SessionSummary};
use crate::tools::transport::TransportState;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipParams {
    /// Track index (0-based)
    pub track: i32,
    /// Clip slot index (0-based)
    pub slot: i32,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Note {
    pub pitch: i32,
    pub start: f32,
    pub duration: f32,
    pub velocity: i32,
    #[serde(default)]
    pub mute: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddNotesParams {
    /// Track index (0-based)
    pub track: i32,
    /// Clip slot index (0-based)
    pub slot: i32,
    /// Array of notes. Max 1000 notes per call.
    pub notes: Vec<Note>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateMidiClipParams {
    /// Track index (0-based)
    pub track: i32,
    /// Clip slot index (0-based)
    pub slot: i32,
    /// Length in beats (e.g., 4.0 for one bar in 4/4)
    pub length: f32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNotesParams {
    /// Track index (0-based)
    pub track: i32,
    /// Clip slot index (0-based)
    pub slot: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateMidiClipWithNotesParams {
    /// Track index (0-based)
    pub track: i32,
    /// Clip slot index (0-based)
    pub slot: i32,
    /// Length in beats (e.g., 4.0 for one bar in 4/4)
    pub length: f32,
    /// Notes to add. Max 1000.
    pub notes: Vec<Note>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClearAndWriteNotesParams {
    /// Track index (0-based)
    pub track: i32,
    /// Clip slot index (0-based)
    pub slot: i32,
    /// Notes to write after clearing. Max 1000.
    pub notes: Vec<Note>,
}

#[derive(Debug, Serialize)]
pub struct ClipNameInfo {
    pub track: i32,
    pub slot: i32,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct NotesResponse {
    pub track: i32,
    pub slot: i32,
    pub note_count: usize,
    pub notes: Vec<Note>,
}

impl AbletonMcpServer {
    pub async fn do_fire_clip(
        &self,
        track: i32,
        slot: i32,
    ) -> Result<(TransportState, SessionSummary), Error> {
        let osc = self.osc().await?;
        osc.send(
            "/live/clip/fire",
            vec![OscType::Int(track), OscType::Int(slot)],
        )
        .await?;
        let summary = common::query_session_summary(osc).await?;
        let state = TransportState {
            is_playing: summary.is_playing,
            tempo: summary.tempo,
        };
        Ok((state, summary))
    }

    pub async fn do_stop_clip(
        &self,
        track: i32,
        slot: i32,
    ) -> Result<SessionSummary, Error> {
        let osc = self.osc().await?;
        osc.send(
            "/live/clip/stop",
            vec![OscType::Int(track), OscType::Int(slot)],
        )
        .await?;
        common::query_session_summary(osc).await
    }

    pub async fn do_get_clip_name(
        &self,
        track: i32,
        slot: i32,
    ) -> Result<(ClipNameInfo, SessionSummary), Error> {
        let osc = self.osc().await?;
        let name: String = osc
            .query_val(
                "/live/clip/get/name",
                vec![OscType::Int(track), OscType::Int(slot)],
            )
            .await?;
        let summary = common::query_session_summary(osc).await?;
        Ok((ClipNameInfo { track, slot, name }, summary))
    }

    pub async fn do_create_midi_clip(
        &self,
        track: i32,
        slot: i32,
        length: f32,
    ) -> Result<SessionSummary, Error> {
        let osc = self.osc().await?;
        osc.send(
            "/live/clip_slot/create_clip",
            vec![
                OscType::Int(track),
                OscType::Int(slot),
                OscType::Float(length),
            ],
        )
        .await?;
        common::query_session_summary(osc).await
    }

    pub async fn do_add_notes(
        &self,
        track: i32,
        slot: i32,
        notes: &[Note],
    ) -> Result<(NotesResponse, SessionSummary), Error> {
        if notes.len() > 1000 {
            return Err(Error::UnexpectedResponse(
                "max 1000 notes per call".into(),
            ));
        }

        let mut args: Vec<OscType> = Vec::with_capacity(2 + notes.len() * 5);
        args.push(OscType::Int(track));
        args.push(OscType::Int(slot));
        for note in notes {
            args.push(OscType::Int(note.pitch));
            args.push(OscType::Float(note.start));
            args.push(OscType::Float(note.duration));
            args.push(OscType::Int(note.velocity));
            args.push(OscType::Int(note.mute));
        }

        let osc = self.osc().await?;
        osc.send("/live/clip/add/notes", args).await?;

        let summary = common::query_session_summary(osc).await?;
        let response = NotesResponse {
            track,
            slot,
            note_count: notes.len(),
            notes: Vec::new(),
        };
        Ok((response, summary))
    }

    pub async fn do_get_notes(
        &self,
        track: i32,
        slot: i32,
    ) -> Result<(NotesResponse, SessionSummary), Error> {
        let osc = self.osc().await?;
        let msg = osc
            .query(
                "/live/clip/get/notes",
                vec![OscType::Int(track), OscType::Int(slot)],
            )
            .await?;

        // Skip first 2 args (track, slot echo), then every 5 values = one note
        let note_args = &msg.args[2..];
        let mut notes = Vec::new();
        for chunk in note_args.chunks(5) {
            if chunk.len() == 5 {
                let pitch = match chunk[0] {
                    OscType::Int(v) => v,
                    _ => continue,
                };
                let start = match chunk[1] {
                    OscType::Float(v) => v,
                    _ => continue,
                };
                let duration = match chunk[2] {
                    OscType::Float(v) => v,
                    _ => continue,
                };
                let velocity = match chunk[3] {
                    OscType::Int(v) => v,
                    _ => continue,
                };
                let mute = match chunk[4] {
                    OscType::Int(v) => v,
                    _ => continue,
                };
                notes.push(Note {
                    pitch,
                    start,
                    duration,
                    velocity,
                    mute,
                });
            }
        }

        let summary = common::query_session_summary(osc).await?;
        let response = NotesResponse {
            track,
            slot,
            note_count: notes.len(),
            notes,
        };
        Ok((response, summary))
    }

    pub async fn do_remove_notes(
        &self,
        track: i32,
        slot: i32,
    ) -> Result<SessionSummary, Error> {
        let osc = self.osc().await?;
        osc.send(
            "/live/clip/remove/notes",
            vec![
                OscType::Int(track),
                OscType::Int(slot),
                OscType::Int(0),
                OscType::Int(128),
                OscType::Float(0.0),
                OscType::Float(100000.0),
            ],
        )
        .await?;
        common::query_session_summary(osc).await
    }

    pub async fn do_create_midi_clip_with_notes(
        &self,
        track: i32,
        slot: i32,
        length: f32,
        notes: &[Note],
    ) -> Result<(NotesResponse, SessionSummary), Error> {
        if notes.len() > 1000 {
            return Err(Error::UnexpectedResponse(
                "max 1000 notes per call".into(),
            ));
        }

        self.do_create_midi_clip(track, slot, length).await?;
        let (response, summary) = self.do_add_notes(track, slot, notes).await?;
        let result = NotesResponse {
            track,
            slot,
            note_count: notes.len(),
            notes: response.notes,
        };
        Ok((result, summary))
    }

    pub async fn do_clear_and_write_notes(
        &self,
        track: i32,
        slot: i32,
        notes: &[Note],
    ) -> Result<(NotesResponse, SessionSummary), Error> {
        if notes.len() > 1000 {
            return Err(Error::UnexpectedResponse(
                "max 1000 notes per call".into(),
            ));
        }

        self.do_remove_notes(track, slot).await?;
        self.do_add_notes(track, slot, notes).await?;
        self.do_get_notes(track, slot).await
    }
}
