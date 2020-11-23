#![no_std]

#[derive(Copy, Clone, Debug)]
pub struct ARGB8 {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Apa102 LED strip buffer
pub struct Apa102 {
    length: usize,
    buffer: Vec<u8>,
}

impl Apa102 {
    /// Create a new Apa102 driver with the given length and SPI bus.
    pub fn new(length: u16) -> Self {
        let end_frame = (6 + length / 16) as usize;
        let led_frame = (4 * (length + 1)) as usize;
        let buffer_size = led_frame + end_frame;
        let mut buffer = vec![0u8; buffer_size];
        buffer[led_frame] = 0xff;
        Self {
            length: length as usize,
            buffer,
        }
    }

    pub fn update(&mut self, frame: &[ARGB8]) {
        let buf = &mut self.buffer;
        for i in 0..self.length {
            let idx = 4 * (1 + i);
            let e = frame[i];
            buf[idx] = 0xE0 | e.a;
            buf[idx + 1] = e.b;
            buf[idx + 2] = e.g;
            buf[idx + 3] = e.r;
        }
    }

    pub fn get_buffer(&self) -> &Vec<u8> {
        &self.buffer
    }
}
