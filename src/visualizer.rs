use std::sync::mpsc::{channel, SyncSender, TrySendError};
use std::thread;

use audio;
use clap::Clap;
use serde::{Deserialize, Serialize};

use crate::apa102::ARGB8;

#[derive(Clap)]
pub struct Opts {
    #[clap(long, short)]
    device: Option<String>,

    #[clap(long, short = 'r', default_value = "44100")]
    sample_rate: usize,

    #[clap(long, short = 'b', default_value = "256")]
    sample_block_size: usize,

    #[clap(long, short = 'f', default_value = "1024")]
    fft_size: usize,

    #[clap(long, short = 'n', default_value = "16")]
    bins: usize,

    #[clap(long, short = 'l', default_value = "144")]
    length: usize,

    #[clap(long)]
    pre_gain: Option<i32>,
}

pub struct Visualizer {
    opts: Opts,
    params: Params,
    verbose: i32,
    sigmoid: Sigmoid,
    clut: Clut,
}

impl Visualizer {
    pub fn new(opts: Opts, params: Params, verbose: i32) -> Self {
        Self {
            opts,
            params,
            verbose,
            sigmoid: Sigmoid::new(),
            clut: Clut::new(),
        }
    }

    pub fn run(
        &self,
        output_size: (usize, usize),
        audio_params: audio::params::FrequencySensorParams,
        frame_tx: SyncSender<Vec<ARGB8>>,
    ) {
        let block_size = self.opts.sample_block_size;
        let fft_size = self.opts.fft_size;
        let bins = self.opts.bins;
        let length = self.opts.length;
        let verbose = self.verbose;

        let (audio_data_tx, audio_data_rx) = channel();
        let (features_tx, features_rx) = channel();

        let now = std::time::SystemTime::now();

        thread::spawn(move || {
            let mut sfft = audio::sfft::SlidingFFT::new(fft_size);
            let mut bucketer =
                audio::bucketer::Bucketer::new(sfft.output_size(), bins, 32.0, 22000.0);

            let mut fs = audio::frequency_sensor::FrequencySensor::new(bins, length, audio_params);
            let mut sample_count = 0;
            let mut fps = 0;

            // let now = std::time::SystemTime::now();

            let mut process = |data| {
                sfft.push_input(&data);
                sample_count += data.len();
                if sample_count >= block_size {
                    sample_count = 0;
                    let frame = sfft.process();
                    let bins = bucketer.bucket(frame);
                    fs.process(bins);
                    let features = fs.get_features();

                    fps += 1;
                    if verbose >= 2 && fps % 32 == 0 {
                        fs.print_debug();
                    }

                    // FIXME: this clone is needlessly expensive on failure to send
                    if let Err(e) = features_tx.send(features.clone()) {
                        if verbose >= 3 {
                            println!(
                                "[{:08}]: failed to send features: {}",
                                now.elapsed().unwrap().as_millis(),
                                e
                            );
                        }
                    }
                }
            };

            loop {
                // match match audio_data_rx.try_recv() {
                //     Ok(data) => Ok(data),
                //     Err(TryRecvError::Empty) => audio_data_rx.recv().map_err(|e| anyhow!(e)),
                //     Err(e) => Err(anyhow!(e)),
                // } {
                match audio_data_rx.recv() {
                    Ok(data) => {
                        process(data);
                    }
                    Err(e) => {
                        println!("failed to recv audio: {}", e);
                        break;
                    }
                };
                if verbose >= 4 {
                    println!("rx audio");
                };
                // thread::sleep(std::time::Duration::from_millis(1000));
            }
        });

        let handle_stream = move |data: &[f32]| {
            if verbose >= 4 {
                println!("tx audio");
            }
            if let Err(e) = audio_data_tx.send(data.to_vec()) {
                if verbose >= 3 {
                    println!(
                        "[{:08}]: failed to send audio data: {}",
                        now.elapsed().unwrap().as_millis(),
                        e
                    );
                }
            }
        };
        // random rust thing:
        // https://stackoverflow.com/questions/25649423/sending-trait-objects-between-threads-in-rust
        let handle_stream = Box::new(handle_stream) as Box<dyn Fn(&[f32]) -> () + Send>;

        let s = audio::Source::new(self.opts.device.as_deref()).expect("failed to get device");
        let _stream = s
            .get_stream(
                1,
                self.opts.sample_rate as u32,
                block_size as u32,
                handle_stream,
            )
            .expect("failed to get stream");

        while let Ok(features) = features_rx.recv() {
            if self.verbose >= 4 {
                println!("features update");
            }
            let frame = self.visualize(output_size, &features);
            if let Err(e) = frame_tx.try_send(frame) {
                match e {
                    TrySendError::Full(_) => {
                        if verbose >= 3 {
                            println!("[{:08}]: dropped frame", now.elapsed().unwrap().as_millis());
                        }
                    }
                    e => {
                        println!("failed to send frame: {}", e);
                        break;
                    }
                };
            }
        }
        println!("oops, dead");
    }

    fn visualize(
        &self,
        output_size: (usize, usize),
        features: &audio::frequency_sensor::Features,
    ) -> Vec<ARGB8> {
        let (length, width) = output_size;
        let mut frame = vec![ARGB8::new(0, 0, 0, 0); length * width];

        let scales = features.get_scales();
        let energy = features.get_energy();
        // let diff = features.get_diff();
        let ws = 2.0 * std::f32::consts::PI / (length as f32);

        for i in 0..length {
            let amp = features.get_amplitudes(i);
            let phi = ws * i as f32;

            for j in 0..width {
                //, ph := range phase {
                let val = scales[j] * (amp[j] - 1.0);
                frame[j * length + i] = self.get_hsv(&self.params, val, energy[j], phi)
            }
        }

        frame
    }

    fn get_hsv(&self, params: &Params, val: f32, e: f32, phi: f32) -> ARGB8 {
        let vs = params.value_scale;
        let ls = params.lightness_scale;
        let als = params.alpha_scale;

        let hue = 0.5 * (params.cycle * phi + e) / std::f32::consts::PI;
        let value = ls.0 * self.sigmoid.f(vs.0 * val + vs.1) + ls.1;
        let alpha = params.max_alpha * self.sigmoid.f(als.0 * val + als.1);

        let color = self.clut.lookup(hue, value);
        ARGB8::new(
            (31.5 * alpha) as u8,
            (255.5 * color.0) as u8,
            (255.5 * color.1) as u8,
            (255.5 * color.2) as u8,
        )
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct Params {
    value_scale: (f32, f32),
    lightness_scale: (f32, f32),
    alpha_scale: (f32, f32),
    max_alpha: f32,
    cycle: f32,
}

impl Params {
    pub fn defaults() -> Self {
        Self {
            value_scale: (1.0, 0.0),
            lightness_scale: (0.76, 0.0),
            alpha_scale: (1.0, -1.0),
            max_alpha: 0.125,
            cycle: 1. / 256.,
        }
    }
}

struct Sigmoid {
    lut: [f32; Self::SIZE],
}

impl Sigmoid {
    const SIZE: usize = 2048;
    const RANGE: f32 = 10.0;
    const SCALE: f32 = Self::SIZE as f32 / (2. * Self::RANGE);

    fn new() -> Self {
        let mut lut = [0.; Self::SIZE];
        let hl = (Self::SIZE / 2) as f32;
        for i in 0..Self::SIZE {
            let x = (i as f32 - hl) / hl * Self::RANGE;
            lut[i] = 1. / (1. + f32::exp(-x));
        }
        Self { lut }
    }

    fn f(&self, x: f32) -> f32 {
        if x >= Self::RANGE {
            self.lut[Self::SIZE - 1]
        } else if x <= -Self::RANGE {
            self.lut[0]
        } else {
            let idx = (x * Self::SCALE) as usize + Self::SIZE / 2;
            self.lut[idx]
        }
    }
}

struct Clut {
    lut: [[(f32, f32, f32); Self::VALUES]; Self::HUES],
}

impl Clut {
    const HUES: usize = 360;
    const VALUES: usize = 256;

    fn new() -> Self {
        use hsluv::hsluv_to_rgb;
        let mut lut = [[(0., 0., 0.); Self::VALUES]; Self::HUES];
        for h in 0..Self::HUES {
            for v in 0..Self::VALUES {
                let c = hsluv_to_rgb((h as f64, 100., 100. * v as f64 / 256.));
                let c = Self::gamma(c);
                lut[h][v] = (c.0 as f32, c.1 as f32, c.2 as f32);
            }
        }
        Self { lut }
    }

    fn gamma(c: (f64, f64, f64)) -> (f64, f64, f64) {
        (c.0 * c.0, c.1 * c.1, c.2 * c.2)
    }

    fn lookup(&self, h: f32, v: f32) -> (f32, f32, f32) {
        let h = (h * Self::HUES as f32) as usize % Self::HUES;
        let v = (v * Self::VALUES as f32) as usize;
        let v = usize::max(usize::min(v, Self::VALUES - 1), 0);
        self.lut[h][v]
    }
}
