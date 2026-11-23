//! cpal audio output for media playback (from DeckCraft): the device pulls mixed samples from the
//! `deckcraft_media::Player`, which makes it the playback clock video frames follow.

use std::sync::{Arc, Mutex, PoisonError};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use deckcraft_media::AudioOut;

/// The first stream error since it was last reported. cpal calls the error callback on its
/// realtime audio thread (on ALSA in a loop while the device stays broken), which must not log:
/// the callback only keeps the first error, and the UI thread logs it when the stream stops.
type ErrorSlot = Arc<Mutex<Option<String>>>;

/// Keep `e` unless an error is already kept. Never blocks: a busy slot drops the error.
fn keep_stream_error(slot: &ErrorSlot, e: &dyn std::fmt::Display) {
    if let Ok(mut kept) = slot.try_lock()
        && kept.is_none()
    {
        *kept = Some(e.to_string());
    }
}

/// The kept error, once (a poisoned lock is tolerated).
fn take_stream_error(slot: &ErrorSlot) -> Option<String> {
    slot.lock().unwrap_or_else(PoisonError::into_inner).take()
}

#[derive(Default)]
pub struct CpalOut {
    stream: Option<cpal::Stream>,
    error: ErrorSlot,
}

impl CpalOut {
    /// Log the stream error the realtime callback kept (UI thread).
    fn report(&self) {
        if let Some(e) = take_stream_error(&self.error) {
            log::warn!("audio stream error: {e}");
        }
    }
}

impl Drop for CpalOut {
    fn drop(&mut self) {
        self.stream = None;
        self.report();
    }
}

impl AudioOut for CpalOut {
    fn start(
        &mut self,
        mut fill: Box<dyn FnMut(&mut [f32], usize, u32) + Send>,
    ) -> Result<(), String> {
        self.stop();
        let dev = cpal::default_host()
            .default_output_device()
            .ok_or("no audio output device")?;
        let cfg = dev.default_output_config().map_err(|e| e.to_string())?;
        let channels = cfg.channels() as usize;
        let rate = cfg.sample_rate().0;
        let config: cpal::StreamConfig = cfg.clone().into();
        let slot = self.error.clone();
        let err = move |e: cpal::StreamError| keep_stream_error(&slot, &e);
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => dev.build_output_stream(
                &config,
                move |buf: &mut [f32], _| fill(buf, channels, rate),
                err,
                None,
            ),
            cpal::SampleFormat::I16 => {
                let mut tmp = Vec::new();
                dev.build_output_stream(
                    &config,
                    move |buf: &mut [i16], _| {
                        tmp.resize(buf.len(), 0.0);
                        fill(&mut tmp, channels, rate);
                        for (o, s) in buf.iter_mut().zip(&tmp) {
                            *o = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                        }
                    },
                    err,
                    None,
                )
            }
            other => return Err(format!("unsupported sample format {other:?}")),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        self.stream = Some(stream);
        Ok(())
    }

    fn stop(&mut self) {
        self.stream = None;
        self.report();
    }
}
