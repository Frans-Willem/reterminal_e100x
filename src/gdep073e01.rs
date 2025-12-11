use crate::displayinterface::{DisplayInterfaceAsync, DisplayInterfaceAsyncError};
use crate::spectra6::{Spectra6Color, SpectraPacker};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::digital::Wait;
use embedded_hal_async::spi::SpiDevice;

const SINGLE_BYTE_WRITE: bool = true;
const IS_BUSY_LOW: bool = true;

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
enum Command {
    PanelSetting = 0x00, // PSR
    PowerSetting = 0x01, // PWRR
    PowerOff = 0x02,
    POFS = 0x03,
    PowerOn = 0x04,
    BoosterSoftStart1 = 0x05, //BTST1
    BoosterSoftStart2 = 0x06, //BTST2
    DeepSleep = 0x07,
    BoosterSoftStart3 = 0x08, // BTST3
    // Missing 0x09-0x0F
    DataStartTransmission = 0x10,
    DisplayRefresh = 0x12,
    PllControl = 0x30, // PLL
    CDI = 0x50,
    TCON_SETTING = 0x60, // TCON
    TRES = 0x61,
    T_VDCS = 0x84,
    PWS = 0xE3,
    CMDH = 0xAA,
}

impl crate::displayinterface::Command for Command {
    fn address(self) -> u8 {
        self as u8
    }
}

pub struct Gdep073e01<SPI, BUSY, DC, RST, DELAY> {
    interface: DisplayInterfaceAsync<SPI, BUSY, DC, RST, DELAY, SINGLE_BYTE_WRITE>,
}

pub struct StateUnknown;
pub struct StateReset;
pub struct StateBusy<T>(T);
pub struct StateInitialized;
pub struct StatePowerOff;

impl<SPI, BUSY, DC, RST, DELAY> Gdep073e01<SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin + Wait,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    pub fn new(_: &mut SPI, busy: BUSY, dc: DC, rst: RST, _: &mut DELAY) -> Self {
        Gdep073e01 {
            interface: DisplayInterfaceAsync::new(busy, dc, rst),
        }
    }

    pub async fn reset(
        &mut self,
        delay: &mut DELAY,
    ) -> Result<(), DisplayInterfaceAsyncError<SPI, BUSY, DC, RST>> {
        self.interface.reset(delay, 10_000, 10_000, 10_000).await
    }

    pub async fn init(
        &mut self,
        spi: &mut SPI,
    ) -> Result<(), DisplayInterfaceAsyncError<SPI, BUSY, DC, RST>> {
        // NOTE: Call after reset
        //self.interface.reset(delay, 10_000, 10_000, 10_000).await?;

        self.interface
            .cmd_with_data(spi, Command::CMDH, &[0x49, 0x55, 0x20, 0x08, 0x09, 0x18])
            .await?;
        self.interface
            .cmd_with_data(spi, Command::PowerSetting, &[0x3F])
            .await?;
        self.interface
            .cmd_with_data(spi, Command::PanelSetting, &[0x5F, 0x69])
            .await?;
        self.interface
            .cmd_with_data(spi, Command::POFS, &[0x00, 0x54, 0x00, 0x44])
            .await?;
        self.interface
            .cmd_with_data(spi, Command::BoosterSoftStart1, &[0x40, 0x1F, 0x1F, 0x2C])
            .await?;
        self.interface
            .cmd_with_data(spi, Command::BoosterSoftStart2, &[0x6F, 0x1F, 0x17, 0x49])
            .await?;
        self.interface
            .cmd_with_data(spi, Command::BoosterSoftStart3, &[0x6F, 0x1F, 0x1F, 0x22])
            .await?;
        self.interface
            .cmd_with_data(spi, Command::PllControl, &[0x03])
            .await?; // esphome does 0x03, example code for 0x08
        self.interface
            .cmd_with_data(spi, Command::CDI, &[0x3F])
            .await?;
        self.interface
            .cmd_with_data(spi, Command::TCON_SETTING, &[0x02, 0x00])
            .await?;
        self.interface
            .cmd_with_data(spi, Command::TRES, &[0x03, 0x20, 0x01, 0xE0])
            .await?;
        self.interface
            .cmd_with_data(spi, Command::T_VDCS, &[0x01])
            .await?;
        self.interface
            .cmd_with_data(spi, Command::PWS, &[0x2F])
            .await?;
        self.interface.cmd(spi, Command::PowerOn).await?;
        self.wait_until_idle().await?;
        Ok(())
    }

    async fn wait_until_idle(
        &mut self,
    ) -> Result<(), DisplayInterfaceAsyncError<SPI, BUSY, DC, RST>> {
        self.interface.wait_until_idle(IS_BUSY_LOW).await
    }

    pub async fn update_frame_raw(
        &mut self,
        spi: &mut SPI,
        data: impl IntoIterator<Item = u8>,
    ) -> Result<(), DisplayInterfaceAsyncError<SPI, BUSY, DC, RST>> {
        self.interface
            .cmd(spi, Command::DataStartTransmission)
            .await?;
        self.interface.data_iter(spi, data).await?;
        Ok(())
    }

    pub async fn update_frame(
        &mut self,
        spi: &mut SPI,
        pixels: impl IntoIterator<Item = Spectra6Color>,
    ) -> Result<(), DisplayInterfaceAsyncError<SPI, BUSY, DC, RST>> {
        self.update_frame_raw(spi, SpectraPacker(pixels.into_iter()))
            .await
    }

    pub async fn display_frame(
        &mut self,
        spi: &mut SPI,
    ) -> Result<(), DisplayInterfaceAsyncError<SPI, BUSY, DC, RST>> {
        self.interface
            .cmd_with_data(spi, Command::DisplayRefresh, &[0x00])
            .await?;
        self.wait_until_idle().await
    }

    pub async fn sleep(
        &mut self,
        spi: &mut SPI,
    ) -> Result<(), DisplayInterfaceAsyncError<SPI, BUSY, DC, RST>> {
        self.interface
            .cmd_with_data(spi, Command::PowerOff, &[0x00])
            .await?;
        self.wait_until_idle().await
    }
}
