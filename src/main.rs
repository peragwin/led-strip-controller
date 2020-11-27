use std::thread;

#[macro_use]
extern crate lazy_static;

use anyhow::Result;
use clap::Clap;
use rppal::spi::{Bus, Mode, SlaveSelect, Spi};
use serde::{Deserialize, Serialize};
use serde_yaml;

use audio::frequency_sensor::FrequencySensorParams;

mod apa102;
use apa102::{Apa102, ARGB8};
mod display;
use display::Display;
mod transform;
use transform::Transform;
mod visualizer;

/// LED Strip Visualizer
#[derive(Clap)]
#[clap(version = "0.1", author = "Steven Cohen <peragwin@gmail.com>")]
struct Opts {
    /// Verbosity, can be used multiple times
    #[clap(short, long, parse(from_occurrences))]
    verbose: i32,

    /// Don't actually load SPI or output anything
    #[clap(short = 'n', long)]
    dry_run: bool,
    /// Number of LEDs in strips
    length: u16,
    /// SPI clock speed in hz
    #[clap(default_value = "4000000")]
    spi_clock: u32,

    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Clap)]
enum Command {
    Init,
    Set(SetOpts),
    Test(TestOpts),
    Visualizer(visualizer::Opts),
}

/// Set all LEDs a single color
#[derive(Clap)]
struct SetOpts {
    /// Red
    red: u8,
    /// Green
    green: u8,
    /// Blue
    blue: u8,
    /// Alpha
    #[clap(default_value = "31")]
    alpha: u8,
}

/// Run tests
#[derive(Clap)]
struct TestOpts {
    /// Test duration in seconds
    #[clap(default_value = "4")]
    duration: u32,
    #[clap(subcommand)]
    cmd: TestCommand,
}

#[derive(Clap)]
enum TestCommand {
    Fps,
    Transform,
    Audio(TestAudioOpts),
}

#[derive(Clap)]
struct TestAudioOpts {
    #[clap(long)]
    show_configs: bool,
    // #[clap(default_value = "default")]
    device: Option<String>,
}

struct App {
    display: Display<ARGB8>,
    config: Config,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
struct Config {
    audio: FrequencySensorParams,
    visualizer: visualizer::Params,
}

impl Config {
    const CONFIG_FILE: &'static str = ".ledconfig.yaml";

    fn default() -> Self {
        Self {
            audio: FrequencySensorParams::defaults(),
            visualizer: visualizer::Params::defaults(),
        }
    }
}

fn setup(opts: &Opts) -> Result<App> {
    let length = opts.length;
    let spi_clock = opts.spi_clock;
    let dry_run = opts.dry_run;
    let verbose = opts.verbose;

    let config = match std::fs::File::open(Config::CONFIG_FILE) {
        Ok(f) => serde_yaml::from_reader(f)?,
        Err(_) => {
            let config = Config::default();
            if let Command::Init = opts.cmd {
                let f = std::fs::File::create(Config::CONFIG_FILE)?;
                serde_yaml::to_writer(f, &config)?;
            };
            config
        }
    };

    let (display, frame_rx) = Display::new();

    thread::spawn(move || {
        let mut fps = 0;
        let mut then = std::time::SystemTime::now();
        let mut print_fps = || {
            fps += 1;
            if verbose > 0 && fps % 256 == 0 {
                let now = std::time::SystemTime::now();
                if let Ok(e) = now.duration_since(then) {
                    then = now;
                    println!("FPS: {}", fps as f64 / e.as_secs_f64());
                }
                fps = 0;
            }
        };

        if dry_run {
            loop {
                if let Err(e) = frame_rx.recv() {
                    println!("error receiving frame: {}", e);
                    break;
                }
                print_fps();
            }
            return;
        }

        let mut spi = Spi::new(Bus::Spi0, SlaveSelect::Ss0, spi_clock, Mode::Mode0)
            .expect("failed to open spi bus");
        let mut leds = Apa102::new(length);
        let transform = Transform::new(4, 144, vec![false, true, false, true], vec![0, 2, 1, 3]);

        while let Ok(frame) = frame_rx.recv() {
            let frame = transform.apply(&frame);
            leds.update(&frame);
            if let Err(e) = spi.write(leds.get_buffer()) {
                println!("failed to write to spi bus: {:}", e);
            }
            print_fps();
        }
        println!("uh-oh, dead");
    });

    Ok(App { display, config })
}

fn main() {
    let opts = Opts::parse();

    let app = setup(&opts).unwrap();

    match opts.cmd {
        Command::Init => (),
        Command::Set(SetOpts {
            red,
            green,
            blue,
            alpha,
        }) => {
            let alpha = if alpha > 31 { 31 } else { alpha };
            let frame = (0..opts.length)
                .map(|_| ARGB8::new(alpha, red, green, blue))
                .collect();
            for _ in 0..2 {
                // write twice to block until the first frame has finished transferring
                app.display.write(&frame).expect("failed to write frame");
            }
        }
        Command::Test(TestOpts { duration, cmd }) => match cmd {
            TestCommand::Fps => {
                // spam frames to check for flickering
                let frame = (0..opts.length).map(|_| ARGB8::new(1, 1, 1, 1)).collect();

                let mut fps = 0;
                use std::time::SystemTime;
                let then = SystemTime::now();
                while {
                    let now = SystemTime::now();
                    now < (then + std::time::Duration::new(duration as u64, 0))
                } {
                    app.display.write(&frame).expect("failed to write frame");
                    fps += 1;
                }
                println!("Fps test of SPI bus: {:?}", fps / duration);
            }
            TestCommand::Transform => {
                let mut fps = 0;
                let l = opts.length;
                use std::time::SystemTime;
                let then = SystemTime::now();
                while {
                    let now = SystemTime::now();
                    now < (then + std::time::Duration::new(duration as u64, 0))
                } {
                    fps += 1;
                    let f = fps / 8;
                    let frame = (0..l)
                        .map(|x| {
                            if x % l == f % l {
                                ARGB8::new(31, 15, 0, 20)
                            } else {
                                ARGB8::new(0, 0, 0, 0)
                            }
                        })
                        .collect();

                    app.display.write(&frame).expect("failed to write frame");
                }
                println!("Fps: {:?}", fps as u32 / duration);
            }
            TestCommand::Audio(TestAudioOpts {
                show_configs,
                device,
            }) => {
                test_audio(duration as u64, show_configs, device.as_deref());
            }
        },
        Command::Visualizer(vopts) => {
            let vis = visualizer::Visualizer::new(vopts, app.config.visualizer, opts.verbose);
            vis.run((144, 4), app.config.audio, app.display.sink());
        }
    };
}

use std::sync::mpsc::channel;

fn test_audio(timeout: u64, show_configs: bool, device: Option<&str>) {
    audio::Source::print_devices(show_configs).expect("failed to print devices");

    let (audio_data_tx, audio_data_rx) = channel();

    let mut sfft = audio::sfft::SlidingFFT::new(1024);
    let mut bucketer = audio::bucketer::Bucketer::new(512, 16, 32.0, 16000.0);
    let mut fs =
        audio::frequency_sensor::FrequencySensor::new(16, 128, FrequencySensorParams::defaults());
    println!("Bucket Indices: {:?}", bucketer.indices);

    thread::spawn(move || {
        let boost_params = audio::gain_control::Params::defaults();
        let fs_params = FrequencySensorParams::defaults();
        let mut analyzer = audio::Analyzer::new(1024, 256, 4, 128, boost_params, fs_params);
        loop {
            if let Ok((t, mut data)) = audio_data_rx.recv() {
                if let Some(features) = analyzer.process(&mut data) {
                    let mut out = String::new();
                    analyzer
                        .write_debug(&mut out)
                        .expect("failed to write fs debug");
                    println!("{}", out);
                }
            } else {
                break;
            }
        }
    });

    let s = audio::Source::new(device).expect("failed to get device");

    let handle_stream = move |data: &[f32]| {
        let now = std::time::SystemTime::now();
        let data = data.iter().map(|&x| x as f64).collect();
        if let Err(e) = audio_data_tx.send((now, data)) {
            println!("failed to send audio data: {}", e);
        }
    };
    // random rust thing:
    // https://stackoverflow.com/questions/25649423/sending-trait-objects-between-threads-in-rust
    let handle_stream = Box::new(handle_stream) as Box<dyn Fn(&[f32]) -> () + Send>;

    let stream = s
        .get_stream(1, 44100, 512, handle_stream)
        .expect("failed to get stream");

    std::thread::sleep(std::time::Duration::from_secs(timeout));
    drop(stream);
}
