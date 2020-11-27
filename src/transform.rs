use crate::apa102::ARGB8;
use crate::display;

pub struct Transform {
    num_strips: u8,
    strip_length: u16,
    reversed: Vec<bool>,
    x_map: Vec<usize>,
}

impl Transform {
    pub fn new(num_strips: u8, strip_length: u16, reversed: Vec<bool>, x_map: Vec<usize>) -> Self {
        let size = num_strips as usize;
        if reversed.len() != size || x_map.len() != size {
            panic!("invalid reverse or x_map. vectors must be exactly size of num_strips");
        }
        Self {
            num_strips,
            strip_length,
            reversed,
            x_map,
        }
    }

    pub fn apply(&self, frame: &Vec<ARGB8>) -> Vec<ARGB8> {
        let l = self.strip_length as usize;
        let n = self.num_strips as usize;
        (0..n)
            .map(|x| (self.x_map[x], self.reversed[x]))
            .map(|x| (x, frame[l * x.0..l * (x.0 + 1)].iter().copied()))
            .map(
                |(x, s)| -> Vec<ARGB8> {
                    if x.1 {
                        s.rev().collect()
                    } else {
                        s.collect()
                    }
                },
            )
            .flatten()
            .collect()
    }
}

impl display::Transform<ARGB8> for Transform {
    fn transform(&self, frame: &Vec<ARGB8>) -> Vec<ARGB8> {
        self.apply(frame)
    }

    fn write_pixel(&self, frame: &mut Vec<ARGB8>, x: usize, y: usize, color: ARGB8) {
        let l = self.strip_length as usize;
        if x > (self.num_strips - 1) as usize || y > l - 1 {
            panic!(
                "invalid {{x:{:},y:{:}}} for {{{:},{:}}}",
                x, y, self.num_strips, self.strip_length
            );
        }
        let (x, rev) = (self.x_map[x], self.reversed[x]);
        let idx = l * x + if rev { l - y } else { y };
        frame[idx] = color;
    }
}
