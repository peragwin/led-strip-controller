use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

use anyhow::{anyhow, Result};

/// Transform from input frame to display frame format.
pub trait Transform<Color> {
    fn transform(&self, frame: &Vec<Color>) -> Vec<Color>;
    fn write_pixel(&self, frame: &mut Vec<Color>, x: usize, y: usize, color: Color);
}

/// Identity transform function
pub struct Identity;

impl<Color> Transform<Color> for Identity
where
    Color: Clone,
{
    fn transform(&self, frame: &Vec<Color>) -> Vec<Color> {
        frame.to_vec()
    }

    fn write_pixel(&self, _: &mut Vec<Color>, _: usize, _: usize, _: Color) {}
}

/// Display manages a display buffer.
pub struct Display<Color> {
    sender: SyncSender<Vec<Color>>,
}

impl<Color> Display<Color>
where
    Color: Copy + Clone,
{
    pub fn new() -> (Self, Receiver<Vec<Color>>) {
        let (sender, receiver) = sync_channel(0);
        (Self { sender }, receiver)
    }

    pub fn write(&self, frame: &Vec<Color>) -> Result<()> {
        self.sender
            .send(frame.clone())
            .map_err(|_| anyhow!("failed to send frame"))
    }

    pub fn sink(&self) -> SyncSender<Vec<Color>> {
        self.sender.clone()
    }
}
