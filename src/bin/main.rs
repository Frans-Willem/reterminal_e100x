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

use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::gpio::{Input, InputPin, InputConfig, Pull};
use esp_println::println;

use esp_hal::spi::master::Config as SpiConfig;
use esp_hal::spi::master::Spi;
use esp_hal::spi::Mode as SpiMode;

use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal::spi::SpiDevice;

use esp_backtrace as _;

use arrayvec::ArrayVec;

extern crate alloc;

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
            inverted
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

#[embassy_executor::task(pool_size=3)]
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

struct Epd<'t, SPIDEV> {
    busy: Input<'t>,
    busy_inverted: bool,
    reset: Output<'t>,
    dc: Output<'t>,
    spi_dev: SPIDEV,
}

impl<'t, SPIDEV> Epd<'t, SPIDEV> {
    // EPD_W21_Init in example code
    pub async fn reset(&mut self) {
       self.reset.set_low();
       Timer::after(Duration::from_millis(10)).await;
       self.reset.set_high();
       Timer::after(Duration::from_millis(10)).await;
        self.wait_ready().await;
    }

    pub async fn wait_ready(&mut self) {
        if self.busy_inverted {
            self.busy.wait_for_high().await
        } else {
            self.busy.wait_for_low().await
        }
    }
}
impl<'t, SPIDEV> Epd<'t, SPIDEV> where SPIDEV: SpiDevice<u8> {
    pub fn write(&mut self, cmd: u8, data: &[u8]) -> Result<(), SPIDEV::Error> {
        self.dc.set_low();
        self.spi_dev.write(&[cmd])?;
        if data.len() > 0 {
            self.dc.set_high();
            self.spi_dev.write(data)?;
        }
        Ok(())
    }

    pub fn write_iter(&mut self, cmd: u8, data: impl IntoIterator<Item=u8>) -> Result<(), SPIDEV::Error> {
        self.dc.set_low();
        self.spi_dev.write(&[cmd])?;
        let mut buffer = ArrayVec::<u8, 32>::new();
        for v in data.into_iter() {
            if buffer.is_full() {
                self.dc.set_high();
                self.spi_dev.write(buffer.as_slice())?;
                buffer.clear();
            }
            buffer.push(v);
        }
        if !buffer.is_empty() {
                self.dc.set_high();
                self.spi_dev.write(buffer.as_slice())?;
                buffer.clear();
        }
        Ok(())
    }

    pub async fn init(&mut self) -> Result<(), SPIDEV::Error> {
        self.reset().await;

        self.write(0xAA,&[0x49,0x55,0x20,0x08,0x09,0x18])?;
        self.write(0x01 /* PWRR */, &[0x3F])?;
        self.write(0x00 /* PSR */, &[0x5F, 0x69])?;
        self.write(0x03 /* POFS */, &[0x00, 0x54, 0x00, 0x44])?;
        self.write(0x05 /* BTST1 */, &[0x40, 0x1F, 0x1F, 0x2C])?;
        self.write(0x06 /* BTST2 */, &[0x6F, 0x1F, 0x17, 0x49])?;
        self.write(0x08 /* BTST3 */, &[0x6F, 0x1F, 0x1F, 0x22])?;
        self.write(0x30 /* PLL */, &[0x03])?; // esphome does 0x03, example code for 0x08
        self.write(0x50 /* CDI */, &[0x3F])?;
        self.write(0x60 /* TCON */, &[0x02, 0x00])?;
        self.write(0x61 /* TRES */, &[0x03, 0x20, 0x01, 0xE0])?;
        self.write(0x84 /* T_VDCS*/, &[0x01])?;
        self.write(0xE3 /* PWS */, &[0x2F])?;
        self.write(0x04 /* PWR on */, &[])?;

        self.wait_ready().await;
        Ok(())
    }

    pub async fn display(&mut self, data: impl IntoIterator<Item=Spectra6Color>) -> Result<(), SPIDEV::Error> {
        self.write_iter(0x10, SpectraPacker(data.into_iter()))?;
        self.write(0x12, &[0x00])?;
        Timer::after(Duration::from_millis(20)).await;
        self.wait_ready().await;
        Ok(())
    }

    pub async fn sleep(&mut self) -> Result<(), SPIDEV::Error> {
        self.write(0x02, &[0x00])?;
        self.wait_ready().await;
        Ok(())
    }
}

enum Spectra6Color {
    Black = 0,
    White = 1,
    Yellow = 2,
    Red = 3,
    Blue = 5,
    Green = 6,
    Clean = 7,
}

struct SpectraPacker<T>(T);

impl<T> Iterator for SpectraPacker<T> where T: Iterator<Item=Spectra6Color> {
    type Item = u8;
    fn next(&mut self) -> Option<Self::Item> {
        let left = self.0.next()?;
        let right = self.0.next().unwrap_or(Spectra6Color::White);
        Some((left as u8) << 4 | (right as u8))
    }
}

fn test_screen(width: usize, height: usize) -> impl Iterator<Item=Spectra6Color> {
    (0 .. width * height).map(move |index| {
        let _x = index % width;
        let y = index / width;
        match (y / 32) % 6 {
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

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.0.1
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    let radio_init = esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");
    let (mut _wifi_controller, _interfaces) =
        esp_radio::wifi::new(&radio_init, peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");

    //let mut led = Output::new(peripherals.GPIO6, Level::High, OutputConfig::default());

    spawner.spawn(button_task(Button::new(peripherals.GPIO3, InputConfig::default().with_pull(Pull::Up), true), "Refresh")).unwrap();
    spawner.spawn(button_task(Button::new(peripherals.GPIO4, InputConfig::default().with_pull(Pull::Up), true), "Right")).unwrap();
    spawner.spawn(button_task(Button::new(peripherals.GPIO5, InputConfig::default().with_pull(Pull::Up), true), "Left")).unwrap();
    spawner.spawn(blink_task(Output::new(peripherals.GPIO6, Level::Low, OutputConfig::default()))).unwrap();

    let epd_spi_bus = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
        .with_write_bit_order(esp_hal::spi::BitOrder::MsbFirst)
        .with_frequency(esp_hal::time::Rate::from_mhz(20))
        .with_mode(SpiMode::_0),
    ).unwrap();
    let epd_spi_bus = epd_spi_bus
        .with_sck(peripherals.GPIO7)
        .with_mosi(peripherals.GPIO9);

    let epd_spi_dev = ExclusiveDevice::new(epd_spi_bus, Output::new(peripherals.GPIO20, Level::Low, OutputConfig::default()), esp_hal::delay::Delay::new()).unwrap();

    let mut epd = Epd {
        busy: Input::new(peripherals.GPIO13, InputConfig::default().with_pull(Pull::Up)),
        busy_inverted: true,
        reset: Output::new(peripherals.GPIO12, Level::Low, OutputConfig::default()),
        dc: Output::new(peripherals.GPIO11, Level::Low, OutputConfig::default()),
        spi_dev: epd_spi_dev,
    };

    println!("init");
    epd.init().await.unwrap();
    println!("display");
    epd.display(test_screen(800,480)).await.unwrap();
    println!("sleep");
    epd.sleep().await.unwrap();
    println!("Done");

      


    // TODO: Spawn some tasks
    let _ = spawner;

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0/examples/src/bin
}
