use crate::displayinterface::{DisplayInterfaceAsync, DisplayInterfaceAsyncError};
use crate::spectra6::{Spectra6Color, SpectraPacker};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::digital::Wait;
use embedded_hal_async::spi::SpiDevice;

const SINGLE_BYTE_WRITE: bool = true;
const IS_BUSY_LOW: bool = true;

#[allow(non_camel_case_types, dead_code)]
#[derive(Copy, Clone)]
// Seems to be similar to UC8159
// Datasheet: https://v4.cecdn.yun300.cn/100001_1909185148/UC8159-1.pdf
// Seems to be similar to SPD1656 (the BTST) settings
// Datasheet: https://www.waveshare.com/w/upload/b/bf/SPD1656_1.1.pdf
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
        Ok(())
    }

    pub async fn wait_until_idle(
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
            .await
        // NOTE: Must wait here
    }
    pub async fn power_on(
        &mut self,
        spi: &mut SPI,
    ) -> Result<(), DisplayInterfaceAsyncError<SPI, BUSY, DC, RST>> {
        self.interface.cmd(spi, Command::PowerOn).await
        // NOTE: Must wait here
    }

    pub async fn power_off(
        &mut self,
        spi: &mut SPI,
    ) -> Result<(), DisplayInterfaceAsyncError<SPI, BUSY, DC, RST>> {
        self.interface
            .cmd_with_data(spi, Command::PowerOff, &[0x00])
            .await
        //NOTE: Must wait here
    }
}

pub struct StateUnknown;
pub struct StateReset;
pub struct StatePowerOff;
pub struct StateBusy<T>(T);
pub struct StatePowerOn;

pub struct Gdep073e01State<STATE, SPI, BUSY, DC, RST, DELAY> {
    display: Gdep073e01<SPI, BUSY, DC, RST, DELAY>,
    state: STATE,
}

#[allow(dead_code)] // Allow display in here, even if it's likely never used.
pub struct Gdep073e01StateError<SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin + Wait,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    display: Gdep073e01State<StateUnknown, SPI, BUSY, DC, RST, DELAY>,
    error: DisplayInterfaceAsyncError<SPI, BUSY, DC, RST>,
}

impl<SPI, BUSY, DC, RST, DELAY> core::fmt::Debug for Gdep073e01StateError<SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin + Wait,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.error.fmt(f)
    }
}

type Gdep073e01StateResult<STATE, SPI, BUSY, DC, RST, DELAY> = Result<
    Gdep073e01State<STATE, SPI, BUSY, DC, RST, DELAY>,
    Gdep073e01StateError<SPI, BUSY, DC, RST, DELAY>,
>;

impl<SPI, BUSY, DC, RST, DELAY> Gdep073e01State<StateUnknown, SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin + Wait,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    pub fn new(_: &mut SPI, busy: BUSY, dc: DC, rst: RST, _: &mut DELAY) -> Self {
        Self {
            display: Gdep073e01 {
                interface: DisplayInterfaceAsync::new(busy, dc, rst),
            },
            state: StateUnknown,
        }
    }
}

impl<STATE, SPI, BUSY, DC, RST, DELAY> Gdep073e01State<STATE, SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin + Wait,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    fn map_state_from_result<R, NEWSTATE, F: FnOnce(STATE, R) -> NEWSTATE>(
        self,
        ret: Result<R, DisplayInterfaceAsyncError<SPI, BUSY, DC, RST>>,
        f: F,
    ) -> Gdep073e01StateResult<NEWSTATE, SPI, BUSY, DC, RST, DELAY> {
        match ret {
            Ok(result) => Ok(Gdep073e01State {
                display: self.display,
                state: f(self.state, result),
            }),
            Err(error) => Err(Gdep073e01StateError {
                display: Gdep073e01State {
                    display: self.display,
                    state: StateUnknown,
                },
                error,
            }),
        }
    }
    pub async fn reset(
        mut self,
        delay: &mut DELAY,
    ) -> Gdep073e01StateResult<StateReset, SPI, BUSY, DC, RST, DELAY> {
        let res = self.display.reset(delay).await;
        self.map_state_from_result(res, |_, _| StateReset)
    }
}

impl<SPI, BUSY, DC, RST, DELAY> Gdep073e01State<StateReset, SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin + Wait,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    pub async fn init(
        mut self,
        spi: &mut SPI,
    ) -> Gdep073e01StateResult<StatePowerOff, SPI, BUSY, DC, RST, DELAY> {
        let res = self.display.init(spi).await;
        self.map_state_from_result(res, |_, _| StatePowerOff)
    }
}

impl<SPI, BUSY, DC, RST, DELAY> Gdep073e01State<StatePowerOff, SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin + Wait,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    pub async fn power_on_no_wait(
        mut self,
        spi: &mut SPI,
    ) -> Gdep073e01StateResult<StateBusy<StatePowerOn>, SPI, BUSY, DC, RST, DELAY> {
        let res = self.display.power_on(spi).await;
        self.map_state_from_result(res, |_, _| StateBusy(StatePowerOn))
    }
    pub async fn power_on(
        self,
        spi: &mut SPI,
    ) -> Gdep073e01StateResult<StatePowerOn, SPI, BUSY, DC, RST, DELAY> {
        self.power_on_no_wait(spi).await?.wait().await
    }
}

impl<DONESTATE, SPI, BUSY, DC, RST, DELAY> Gdep073e01State<StateBusy<DONESTATE>, SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin + Wait,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    pub async fn wait(
        mut self,
    ) -> Gdep073e01StateResult<DONESTATE, SPI, BUSY, DC, RST, DELAY> {
        let res = self.display.wait_until_idle().await;
        self.map_state_from_result(res, |StateBusy(x), _| x)
    }
}

impl<SPI, BUSY, DC, RST, DELAY> Gdep073e01State<StatePowerOn, SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin + Wait,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    pub async fn power_off_no_wait(
        mut self,
        spi: &mut SPI,
    ) -> Gdep073e01StateResult<StateBusy<StatePowerOff>, SPI, BUSY, DC, RST, DELAY> {
        let res = self.display.power_off(spi).await;
        self.map_state_from_result(res, |_, _| StateBusy(StatePowerOff))
    }

    pub async fn power_off(
        self, spi: & mut SPI) -> Gdep073e01StateResult<StatePowerOff, SPI, BUSY, DC, RST, DELAY> {
        self.power_off_no_wait(spi).await?
        .wait().await
    }

    pub async fn update_frame(
        mut self,
        spi: &mut SPI,
        pixels: impl IntoIterator<Item = Spectra6Color>,
    ) -> Gdep073e01StateResult<StatePowerOn, SPI, BUSY, DC, RST, DELAY> {
        let res = self.display.update_frame(spi, pixels).await;
        self.map_state_from_result(res, |s, _| s)
    }

    pub async fn display_frame_no_wait(
        mut self,
        spi: &mut SPI,
    ) -> Gdep073e01StateResult<StateBusy<StatePowerOn>, SPI, BUSY, DC, RST, DELAY> {
        let res = self.display.display_frame(spi).await;
        self.map_state_from_result(res, |s, _| StateBusy(s))
    }
    pub async fn display_frame(
        self,
        spi: &mut SPI,
    ) -> Gdep073e01StateResult<StatePowerOn, SPI, BUSY, DC, RST, DELAY> {
        self.display_frame_no_wait(spi).await?.wait().await
    }
}

