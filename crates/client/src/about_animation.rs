use image::AnimationDecoder;
use std::{
    io::Cursor,
    time::{Duration, Instant},
};

const ABOUT_ANIMATION_BYTES: &[u8] = include_bytes!("../../../assets/images/about.webp");

pub struct AboutAnimation {
    frames: image::Frames<'static>,
    current: Option<AboutAnimationFrame>,
    loop_started_at: Instant,
    next_frame_at: Duration,
    next_frame_id: u64,
}

pub struct AboutAnimationFrame {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub id: u64,
}

impl AboutAnimation {
    pub fn decode() -> Result<Self, String> {
        let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(ABOUT_ANIMATION_BYTES))
            .map_err(|error| format!("could not decode the bundled About animation: {error}"))?;
        if !decoder.has_animation() {
            return Err("the bundled About animation contains no animation frames".to_owned());
        }
        Ok(Self {
            frames: decoder.into_frames(),
            current: None,
            loop_started_at: Instant::now(),
            next_frame_at: Duration::ZERO,
            next_frame_id: 0,
        })
    }

    pub fn current_frame(&mut self) -> Result<&AboutAnimationFrame, String> {
        let elapsed = self.loop_started_at.elapsed();
        self.current_frame_at(elapsed)
    }

    fn current_frame_at(&mut self, elapsed: Duration) -> Result<&AboutAnimationFrame, String> {
        if self.current.is_none() {
            self.load_next_frame()?;
            // Decoding a WebP frame can take longer than its display duration.
            // Do not try to catch up by decoding the whole backlog in this UI
            // call; the next call can advance by one more frame.
            return self
                .current
                .as_ref()
                .ok_or_else(|| "the bundled About animation contains no frames".to_owned());
        }

        if elapsed >= self.next_frame_at {
            if self.load_next_frame()? {
                return self
                    .current
                    .as_ref()
                    .ok_or_else(|| "the bundled About animation contains no frames".to_owned());
            }
            self.restart()?;
        }

        self.current
            .as_ref()
            .ok_or_else(|| "the bundled About animation contains no frames".to_owned())
    }

    fn load_next_frame(&mut self) -> Result<bool, String> {
        let Some(frame) = self.frames.next() else {
            return Ok(false);
        };
        let frame =
            frame.map_err(|error| format!("could not read the About animation: {error}"))?;
        let duration = frame.delay().numer_denom_ms();
        let duration =
            Duration::from_secs_f64(duration.0 as f64 / duration.1.max(1) as f64 / 1_000.0)
                .max(Duration::from_millis(1));
        let image = frame.into_buffer();
        self.next_frame_at += duration;
        self.current = Some(AboutAnimationFrame {
            width: image.width() as usize,
            height: image.height() as usize,
            pixels: image.into_raw(),
            id: self.next_frame_id,
        });
        self.next_frame_id = self.next_frame_id.wrapping_add(1);
        Ok(true)
    }

    fn restart(&mut self) -> Result<(), String> {
        let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(ABOUT_ANIMATION_BYTES))
            .map_err(|error| format!("could not restart the bundled About animation: {error}"))?;
        self.frames = decoder.into_frames();
        self.current = None;
        self.loop_started_at = Instant::now();
        self.next_frame_at = Duration::ZERO;
        self.load_next_frame()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_frame_does_not_catch_up_while_it_decodes() {
        let mut animation = AboutAnimation::decode().unwrap();
        let frame = animation.current_frame_at(Duration::from_secs(60)).unwrap();
        assert_eq!(frame.id, 0);
    }
}
