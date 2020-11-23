use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

use anyhow::{anyhow, Result};

/// Transform from input frame to display frame format.
pub trait Transform<Color> {
    fn transform(frame: &Vec<Color>) -> &Vec<Color>;
}

/// Identity transform function
pub struct Identity;

impl<Color> Transform<Color> for Identity {
    fn transform(frame: &Vec<Color>) -> &Vec<Color> {
        frame
    }
}

/// Display manages a display buffer.
pub struct Display<Color, T>
where
    T: Transform<Color>,
{
    _transform: T,
    sender: SyncSender<Vec<Color>>,
}

impl<Color, T> Display<Color, T>
where
    Color: Copy + Clone,
    T: Transform<Color>,
{
    pub fn new(transform: T) -> (Self, Receiver<Vec<Color>>) {
        let (sender, receiver) = sync_channel(0);
        (
            Self {
                _transform: transform,
                sender,
            },
            receiver,
        )
    }

    pub fn write(&self, frame: &Vec<Color>) -> Result<()> {
        let frame = T::transform(frame).to_owned();
        self.sender
            .send(frame)
            .map_err(|_| anyhow!("failed to send frame"))
    }
}
