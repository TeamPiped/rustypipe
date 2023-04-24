use serde::{Deserialize, Serialize};

/// Set of video frames for seek preview
///
/// YouTube generates a set of images containing a grid of frames for each video.
/// These images are used by the player for the seekbar preview.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Frameset {
    /// Url template of the frameset
    ///
    /// The `$M` placeholder has to be replaced with the page index (starting from 0).
    pub url_template: String,
    /// Width of a single frame in pixels
    pub frame_width: u32,
    /// Height of a single frame in pixels
    pub frame_height: u32,
    /// Number of pages (individual images)
    pub page_count: u32,
    /// Total number of frames in the set
    pub total_count: u32,
    /// Duration per frame in milliseconds
    pub duration_per_frame: u32,
    /// Number of frames in the x direction
    pub frames_per_page_x: u32,
    /// Number of frames in the y direction.
    pub frames_per_page_y: u32,
}

/// Iterator producing frameset page urls
pub struct FramesetUrls<'a> {
    frameset: &'a Frameset,
    i: u32,
}

impl Frameset {
    /// Gets an iterator over the page URLs of the frameset
    pub fn urls(&self) -> FramesetUrls {
        FramesetUrls {
            frameset: self,
            i: 0,
        }
    }
}

impl Iterator for FramesetUrls<'_> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i < self.frameset.page_count {
            let url = self
                .frameset
                .url_template
                .replace("$M", &self.i.to_string());
            self.i += 1;
            Some(url)
        } else {
            None
        }
    }
}
