#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;

use esp_hal::gpio::{Input, InputConfig, InputPin, Pull};
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_println::println;

use esp_hal::spi::Mode as SpiMode;
use esp_hal::spi::master::Config as SpiConfig;
use esp_hal::spi::master::Spi;

use embedded_graphics::pixelcolor::{BinaryColor, Rgb888};
use embedded_hal_bus::spi::ExclusiveDevice;

use esp_backtrace as _;

extern crate alloc;

use reterminal_e100x::gdep073e01::Gdep073e01State;
use reterminal_e100x::spectra6::Spectra6Color;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

struct Button<'t> {
    input: Input<'t>,
    inverted: bool,
}

impl<'t> Button<'t> {
    pub fn new(pin: impl InputPin + 't, config: InputConfig, inverted: bool) -> Self {
        Button {
            input: Input::new(pin, config),
            inverted,
        }
    }
    pub fn is_pressed(&self) -> bool {
        if self.inverted {
            self.input.is_low()
        } else {
            self.input.is_high()
        }
    }

    pub fn is_released(&self) -> bool {
        !self.is_pressed()
    }

    pub async fn wait_for(&mut self, pressed: bool) {
        if self.inverted ^ pressed {
            self.input.wait_for_high().await
        } else {
            self.input.wait_for_low().await
        }
    }

    pub async fn wait_for_pressed(&mut self) {
        self.wait_for(true).await
    }

    pub async fn wait_for_released(&mut self) {
        self.wait_for(false).await
    }
}

#[embassy_executor::task(pool_size = 3)]
async fn button_task(mut button: Button<'static>, button_name: &'static str) {
    loop {
        loop {
            button.wait_for_pressed().await;
            Timer::after(Duration::from_millis(10)).await; // debounce
            if button.is_pressed() {
                break;
            }
        }
        println!("Button {0} pressed!", button_name);
        loop {
            button.wait_for_released().await;
            Timer::after(Duration::from_millis(10)).await; // debounce
            if button.is_released() {
                break;
            }
        }
        println!("Button {0} released!", button_name);
    }
}

#[embassy_executor::task]
async fn blink_task(mut led: Output<'static>) {
    loop {
        //println!("Toggle LED!");
        led.toggle();
        Timer::after(Duration::from_millis(500)).await;
    }
}

fn test_screen(width: usize, height: usize) -> impl Iterator<Item = Spectra6Color> {
    (0..width * height).map(move |index| {
        let x = index % width;
        let y = index / width;
        match ((x / 32) + (y / 32)) % 6 {
            0 => Spectra6Color::White,
            1 => Spectra6Color::Black,
            2 => Spectra6Color::Red,
            3 => Spectra6Color::Green,
            4 => Spectra6Color::Blue,
            5 => Spectra6Color::Yellow,
            _ => Spectra6Color::White,
        }
    })
}

const TEST_PNG: &[u8] = include_bytes!("test.png");

// TODO: Make a second palette where I stretch all colors such that white is #FFFFFF, and black is
// #000000

const SPECTRA_6_PALETTE: &[(Rgb888, Spectra6Color)] = &[
    (Rgb888::new(0x19, 0x1E, 0x21), Spectra6Color::Black),
    (Rgb888::new(0xE8, 0xE8, 0xE8), Spectra6Color::White),
    (Rgb888::new(0x21, 0x57, 0xBA), Spectra6Color::Blue),
    (Rgb888::new(0x12, 0x5F, 0x20), Spectra6Color::Green),
    (Rgb888::new(0xB2, 0x13, 0x18), Spectra6Color::Red),
    (Rgb888::new(0xEF, 0xDE, 0x44), Spectra6Color::Yellow),
];

const SPECTRA_6_PALETTE_SATURATED: &[(Rgb888, Spectra6Color)] = &[
    (Rgb888::new(0, 0, 0), Spectra6Color::Black),
    (Rgb888::new(255, 255, 255), Spectra6Color::White),
    (Rgb888::new(33, 87, 186), Spectra6Color::Blue),
    (Rgb888::new(18, 95, 32), Spectra6Color::Green),
    (Rgb888::new(178, 19, 24), Spectra6Color::Red),
    (Rgb888::new(239, 222, 68), Spectra6Color::Yellow),
];

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.0.1
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    let radio_init = esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");
    let (mut _wifi_controller, _interfaces) =
        esp_radio::wifi::new(&radio_init, peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");

    spawner
        .spawn(button_task(
            Button::new(
                peripherals.GPIO3,
                InputConfig::default().with_pull(Pull::Up),
                true,
            ),
            "Refresh",
        ))
        .unwrap();
    spawner
        .spawn(button_task(
            Button::new(
                peripherals.GPIO4,
                InputConfig::default().with_pull(Pull::Up),
                true,
            ),
            "Right",
        ))
        .unwrap();
    spawner
        .spawn(button_task(
            Button::new(
                peripherals.GPIO5,
                InputConfig::default().with_pull(Pull::Up),
                true,
            ),
            "Left",
        ))
        .unwrap();
    spawner
        .spawn(blink_task(Output::new(
            peripherals.GPIO6,
            Level::Low,
            OutputConfig::default(),
        )))
        .unwrap();

    let epd_spi_bus = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_write_bit_order(esp_hal::spi::BitOrder::MsbFirst)
            .with_frequency(esp_hal::time::Rate::from_mhz(20))
            .with_mode(SpiMode::_0),
    )
    .unwrap();
    let epd_spi_bus = epd_spi_bus
        .with_sck(peripherals.GPIO7)
        .with_mosi(peripherals.GPIO9)
        .into_async();

    let mut epd_spi_dev = ExclusiveDevice::new(
        epd_spi_bus,
        Output::new(peripherals.GPIO20, Level::Low, OutputConfig::default()),
        embassy_time::Delay,
    )
    .unwrap();

    let epd = Gdep073e01State::new(
        &mut epd_spi_dev,
        Input::new(
            peripherals.GPIO13,
            InputConfig::default().with_pull(Pull::Up),
        ),
        Output::new(peripherals.GPIO11, Level::Low, OutputConfig::default()),
        Output::new(peripherals.GPIO12, Level::Low, OutputConfig::default()),
        &mut embassy_time::Delay,
    );

    println!("Decode PNG");
    let (header, data) = png_decoder::decode(TEST_PNG).unwrap();
    println!("Header: {:?}", header);
    let data = data.into_iter();
    let data = data.map(|[r, g, b, _]| Rgb888::new(r, g, b));
    // Color
    let data = reterminal_e100x::dither::FloydSteinberg::new(
        reterminal_e100x::dither::RgbColorToPalette::new(SPECTRA_6_PALETTE_SATURATED),
        data,
        800,
    );
    /*
    // B&W
    let data = reterminal_e100x::dither::FloydSteinberg::new(
        reterminal_e100x::dither::RgbColorToBinaryColor::new(),
        data,
        800,
    );
    let data = data.map(|b| match b {
        BinaryColor::On => Spectra6Color::White,
        BinaryColor::Off => Spectra6Color::Black,
    });
    */

    println!("Reset");
    let epd = epd.reset(&mut embassy_time::Delay).await.unwrap();
    println!("Init");
    let epd = epd.init(&mut epd_spi_dev).await.unwrap();
    println!("Power on");
    let epd = epd.power_on(&mut epd_spi_dev).await.unwrap();
    println!("Update frame");
    let epd = epd.update_frame(&mut epd_spi_dev, data).await.unwrap();
    println!("Display frame");
    let epd = epd.display_frame(&mut epd_spi_dev).await.unwrap();
    println!("Power off");
    let epd = epd.power_off(&mut epd_spi_dev).await.unwrap();
    println!("Done");
    let _ = epd;

    // TODO: Spawn some tasks
    let _ = spawner;

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0/examples/src/bin
}
