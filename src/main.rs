use std::thread;

use anyhow::Result;
use clap::Clap;
use rppal::spi::{Bus, Mode, SlaveSelect, Spi};

mod apa102;
use apa102::{Apa102, ARGB8};

mod display;
use display::Display;

/// LED Strip Visualizer
#[derive(Clap)]
#[clap(version = "0.1", author = "Steven Cohen <peragwin@gmail.com>")]
struct Opts {
    /// Verbosity, can be used multiple times
    #[clap(short, long, parse(from_occurrences))]
    verbose: i32,

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
    Set(SetOpts),
    /// Run tests
    Test,
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

// /// Run tests
// #[derive(Clap)]
// struct TestOpts {}

struct App {
    display: Display<ARGB8, display::Identity>,
    buffer: Vec<ARGB8>,
}

fn setup(opts: &Opts) -> Result<App> {
    let length = opts.length;
    let spi_clock = opts.spi_clock;

    let (display, frame_rx) = Display::new(display::Identity {});

    thread::spawn(move || {
        let mut spi = Spi::new(Bus::Spi0, SlaveSelect::Ss0, spi_clock, Mode::Mode0)
            .expect("failed to open spi bus");
        let mut leds = Apa102::new(length);

        while let Ok(frame) = frame_rx.recv() {
            leds.update(&frame);
            if let Err(e) = spi.write(leds.get_buffer()) {
                println!("failed to write to spi bus: {:}", e);
            }
        }
    });

    Ok(App {
        display,
        buffer: Vec::new(),
    })
}

fn main() {
    let opts = Opts::parse();

    let app = setup(&opts).unwrap();

    match opts.cmd {
        Command::Set(SetOpts {
            red,
            green,
            blue,
            alpha,
        }) => {
            let alpha = if alpha > 31 { 31 } else { alpha };
            let frame = (0..opts.length)
                .map(|_| ARGB8 {
                    a: alpha,
                    r: red,
                    g: green,
                    b: blue,
                })
                .collect();
            app.display.write(&frame).expect("failed to write frame");
            thread::sleep(std::time::Duration::from_millis(500));
        }
        Command::Test => {
            let mut spi =
                Spi::new(Bus::Spi0, SlaveSelect::Ss0, opts.spi_clock, Mode::Mode0).unwrap();

            let led_frame: Vec<u8> = (0..opts.length + 6 + (opts.length / 16))
                .map(|_| (0..4).collect::<Vec<u8>>())
                .flatten()
                .collect();

            let mut fps = 0;
            use std::time::SystemTime;
            let then = SystemTime::now();
            while {
                let now = SystemTime::now();
                now < (then + std::time::Duration::new(4, 0))
            } {
                spi.write(&led_frame).unwrap();
                // app.leds.write((0..opts.length).map(|_| BLACK)).unwrap();
                fps += 1;
            }
            println!("Raw fps test of SPI bus: {:?}", fps / 4);
        }
    };
}
