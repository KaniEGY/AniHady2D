//! Audio timeline operations; no audio data is modified when clips are moved.
use crate::project::AudioTrack;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackWindow {
    pub skip_secs: f64,
    pub delay_secs: f64,
    pub audible_secs: f64,
}

pub fn playback_window(track: &AudioTrack, current_frame: usize, fps: u32) -> Option<PlaybackWindow> {
    if track.muted || track.gain <= 0.0 || current_frame > track.end_frame { return None; }
    let fps = fps.max(1) as f64;
    let skip_secs = current_frame.saturating_sub(track.start_frame) as f64 / fps;
    let delay_secs = track.start_frame.saturating_sub(current_frame) as f64 / fps;
    let duration_secs = (track.end_frame.saturating_sub(track.start_frame) + 1) as f64 / fps;
    let audible_secs = duration_secs - skip_secs;
    (audible_secs > 0.0).then_some(PlaybackWindow { skip_secs, delay_secs, audible_secs })
}

pub fn duplicate_clip(tracks: &mut Vec<AudioTrack>, index: usize, max_frames: usize) -> Option<usize> {
    let original = tracks.get(index)?.clone();
    let duration = original.end_frame.saturating_sub(original.start_frame);
    if max_frames == 0 || duration >= max_frames { return None; }
    let mut copy = original.clone();
    copy.name = if original.name.trim().is_empty() {
        "Audio Copy".to_string()
    } else {
        format!("{} Copy", original.name)
    };
    let preferred = original.end_frame.saturating_add(1);
    let fallback = original.start_frame.saturating_add(1);
    let requested = if preferred.saturating_add(duration) <= max_frames { preferred } else { fallback };
    let _ = move_clip(&mut copy, requested, max_frames);
    tracks.push(copy);
    Some(tracks.len() - 1)
}

pub fn move_clip(track: &mut AudioTrack, requested_start: usize, max_frames: usize) -> bool {
    let duration = track.end_frame.saturating_sub(track.start_frame);
    if duration >= max_frames || max_frames == 0 { return false; }
    let next_start = requested_start.clamp(1, max_frames - duration);
    if track.start_frame == next_start { return false; }
    track.start_frame = next_start;
    track.end_frame = next_start + duration;
    true
}

/// Translate the audio lane's horizontal pointer movement to an integer
/// start frame. Uses round-to-nearest so slow drags work as soon as the
/// pointer travels half a cell, independently of mouse velocity.
pub fn drag_target_frame(origin: usize, delta_px: f32, cell_px: f32, max_frames: usize) -> Option<usize> {
    if !delta_px.is_finite() || !cell_px.is_finite() || cell_px <= 0.0 || max_frames == 0 {
        return None;
    }
    let shift=(delta_px / cell_px).round() as i64;
    Some(((origin as i64).saturating_add(shift)).clamp(1,max_frames as i64) as usize)
}

pub fn peak_for_frame(peaks: &[f32], relative_frame: usize, length: usize) -> f32 {
    if peaks.is_empty() || length == 0 { return 0.0; }
    let sample = (relative_frame as u64 * peaks.len() as u64 / length as u64)
        .min((peaks.len() - 1) as u64) as usize;
    peaks[sample].clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::AudioSyncMode;

    fn track() -> AudioTrack {
        AudioTrack {name: "test".to_string(), file_path: "sample.wav".into(),
            start_frame: 5, end_frame: 25, sync_mode: AudioSyncMode::Stream,
            waveform_peaks: vec![0.2,0.8,0.1], gain: 1.0, muted: false }
    }

    #[test]
    fn playback_window_schedules_overlapping_tracks_independently() {
        let mut a=track();
        a.start_frame=1; a.end_frame=50;
        let mut b=track();
        b.start_frame=20; b.end_frame=80;
        let wa=playback_window(&a,10,10).unwrap();
        let wb=playback_window(&b,10,10).unwrap();
        assert!((wa.skip_secs-0.9).abs()<0.0001);
        assert_eq!(wa.delay_secs,0.0);
        assert!((wb.delay_secs-1.0).abs()<0.0001);
        assert_eq!(wb.skip_secs,0.0);
    }

    #[test]
    fn duplicate_audio_keeps_duration_and_selectable_copy() {
        let mut tracks=vec![track()];
        let idx=duplicate_clip(&mut tracks,0,200).unwrap();
        assert_eq!(idx,1);
        assert_eq!(tracks[1].end_frame-tracks[1].start_frame,20);
        assert!(tracks[1].name.ends_with("Copy"));
        assert!(tracks[1].start_frame>tracks[0].start_frame);
    }

    #[test]
    fn clip_move_keeps_duration_and_never_crosses_canvas_limit() {
        let mut t = track();
        assert!(move_clip(&mut t,12,100));
        assert_eq!((t.start_frame,t.end_frame),(12,32));
        assert!(!move_clip(&mut t,12,100));
        assert!(move_clip(&mut t,95,100));
        assert_eq!((t.start_frame,t.end_frame),(80,100));
        assert!(move_clip(&mut t,0,100));
        assert_eq!((t.start_frame,t.end_frame),(1,21));
    }

    #[test]
    fn slow_audio_drag_is_velocity_independent_and_clamped() {
        assert_eq!(drag_target_frame(20, 0.49, 1.0, 100),Some(20));
        assert_eq!(drag_target_frame(20, 0.51, 1.0, 100),Some(21));
        assert_eq!(drag_target_frame(20, -0.51, 1.0, 100),Some(19));
        assert_eq!(drag_target_frame(20, 12.0, 1.0, 25),Some(25));
        assert_eq!(drag_target_frame(5, -5000.0, 10.0, 100),Some(1));
        assert_eq!(drag_target_frame(20, f32::NAN, 5.0, 100),None);
    }

    #[test]
    fn drag_tracks_one_two_and_ten_frames_across_zoom_levels() {
        for cell_px in [4.0_f32, 12.5, 30.0, 64.0] {
            assert_eq!(drag_target_frame(50, cell_px, cell_px, 500), Some(51));
            assert_eq!(drag_target_frame(50, cell_px * 2.0, cell_px, 500), Some(52));
            assert_eq!(drag_target_frame(50, cell_px * 10.0, cell_px, 500), Some(60));
            assert_eq!(drag_target_frame(50, -cell_px, cell_px, 500), Some(49));
            assert_eq!(drag_target_frame(50, -cell_px * 10.0, cell_px, 500), Some(40));
        }
    }

    #[test]
    fn waveform_mapping_is_bounded() {
        let p = vec![0.0,0.2,0.8,0.1];
        assert_eq!(peak_for_frame(&p,0,100),0.0);
        assert_eq!(peak_for_frame(&p,99,100),0.1);
        assert_eq!(peak_for_frame(&p,50,100),0.8);
        assert_eq!(peak_for_frame(&[],20,100),0.0);
    }
}
