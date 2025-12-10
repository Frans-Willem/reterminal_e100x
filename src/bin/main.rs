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

use esp_backtrace as _;

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

    let mut led = Output::new(peripherals.GPIO6, Level::High, OutputConfig::default());

    spawner.spawn(button_task(Button::new(peripherals.GPIO3, InputConfig::default().with_pull(Pull::Up), true), "Refresh")).unwrap();
    spawner.spawn(button_task(Button::new(peripherals.GPIO4, InputConfig::default().with_pull(Pull::Up), true), "Right")).unwrap();
    spawner.spawn(button_task(Button::new(peripherals.GPIO5, InputConfig::default().with_pull(Pull::Up), true), "Left")).unwrap();

    // TODO: Spawn some tasks
    let _ = spawner;

    loop {
        println!("Toggle me!");
        led.toggle();
        Timer::after(Duration::from_millis(500)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0/examples/src/bin
}
