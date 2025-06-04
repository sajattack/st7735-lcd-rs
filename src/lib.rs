#![no_std]

//! This crate provides a ST7735 driver to connect to TFT displays.

pub mod instruction;

use crate::instruction::Instruction;

use embedded_hal::delay::DelayNs;
use embedded_hal::spi;
use embedded_hal::digital::OutputPin;

/// ST7735 driver to connect to TFT displays.
/// requires const generic BUFSIZE to be specified for write_words_buffered
/// 32 was the previous default
pub struct ST7735<SPI, DC, RST, const BUFSIZE: usize>
where
    SPI: spi::SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
{
    /// SPI
    spi: SPI,

    /// Data/command pin.
    dc: DC,

    /// Reset pin.
    rst: Option<RST>,

    /// Whether the display is RGB (true) or BGR (false)
    rgb: bool,

    /// Whether the colours are inverted (true) or not (false)
    inverted: bool,

    /// Global image offset
    dx: u16,
    dy: u16,
    width: u32,
    height: u32,
}

/// Display orientation.
#[derive(Clone, Copy, Debug)]
pub enum Orientation {
    Portrait = 0x00,
    Landscape = 0x60,
    PortraitSwapped = 0xC0,
    LandscapeSwapped = 0xA0,
}

/// Errors
#[derive(Clone, Copy, Debug)]
pub enum Error<SE, GE>{
    Spi(SE),
    Gpio(GE),
}

impl<SPI, DC, RST, SE, GE, const BUFSIZE: usize> ST7735<SPI, DC, RST, BUFSIZE>
where
    SPI: spi::SpiDevice<Error = SE>,
    DC: OutputPin<Error = GE>,
    RST: OutputPin<Error = GE>,
    SE: embedded_hal::spi::Error,
    GE: embedded_hal::digital::Error,
{
    /// Creates a new driver instance that uses hardware SPI.
    pub fn new(
        spi: SPI,
        dc: DC,
        rst: Option<RST>,
        rgb: bool,
        inverted: bool,
        width: u32,
        height: u32,
    ) -> Self {
        ST7735::<SPI, DC, RST, BUFSIZE>
        {
            spi,
            dc,
            rst,
            rgb,
            inverted,
            dx: 0,
            dy: 0,
            width,
            height,
        }
    }

    /// Runs commands to initialize the display.
    pub fn init<DELAY>(&mut self, delay: &mut DELAY) -> Result<(), Error<SE, GE>> 
    where
        DELAY: DelayNs,
    {
        self.hard_reset(delay)?;
        self.write_command(Instruction::SWRESET, &[])?;
        delay.delay_ms(200);
        self.write_command(Instruction::SLPOUT, &[])?;
        delay.delay_ms(200);
        self.write_command(Instruction::FRMCTR1, &[0x01, 0x2C, 0x2D])?;
        self.write_command(Instruction::FRMCTR2, &[0x01, 0x2C, 0x2D])?;
        self.write_command(Instruction::FRMCTR3, &[0x01, 0x2C, 0x2D, 0x01, 0x2C, 0x2D])?;
        self.write_command(Instruction::INVCTR, &[0x07])?;
        self.write_command(Instruction::PWCTR1, &[0xA2, 0x02, 0x84])?;
        self.write_command(Instruction::PWCTR2, &[0xC5])?;
        self.write_command(Instruction::PWCTR3, &[0x0A, 0x00])?;
        self.write_command(Instruction::PWCTR4, &[0x8A, 0x2A])?;
        self.write_command(Instruction::PWCTR5, &[0x8A, 0xEE])?;
        self.write_command(Instruction::VMCTR1, &[0x0E])?;
        if self.inverted {
            self.write_command(Instruction::INVON, &[])?;
        } else {
            self.write_command(Instruction::INVOFF, &[])?;
        }
        if self.rgb {
            self.write_command(Instruction::MADCTL, &[0x00])?;
        } else {
            self.write_command(Instruction::MADCTL, &[0x08])?;
        }
        self.write_command(Instruction::COLMOD, &[0x05])?;
        self.write_command(Instruction::DISPON, &[])?;
        delay.delay_ms(200);
        Ok(())
    }

    /// Hard-Reset the display
    pub fn hard_reset<DELAY>(&mut self, delay: &mut DELAY) -> Result<(), Error<SE, GE>>
    where
        DELAY: DelayNs,
    {
        if let Some(rst) = &mut self.rst {
            rst.set_high().map_err(|e| Error::Gpio(e))?;
            delay.delay_ms(10);
            rst.set_low().map_err(|e| Error::Gpio(e))?;
            delay.delay_ms(10);
            rst.set_high().map_err(|e| Error::Gpio(e))?;
        }
        Ok(())
    }

    fn write_command(&mut self, command: Instruction, params: &[u8]) -> Result<(), Error<SE, GE>> {
        self.dc.set_low().map_err(|e| Error::Gpio(e))?;
        self.spi.write(&[command as u8]).map_err(|e| Error::Spi(e))?;
        if !params.is_empty() {
            self.start_data()?;
            self.write_data(params)?;
        }
        Ok(())
    }

    fn start_data(&mut self) -> Result<(), Error<SE, GE>> {
        self.dc.set_high().map_err(|e| Error::Gpio(e))
    }

    fn write_data(&mut self, data: &[u8]) -> Result<(), Error<SE, GE>> {
        self.spi.write(data).map_err(|e| Error::Spi(e))
    }

    /// Writes a data word to the display.
    fn write_word(&mut self, value: u16) -> Result<(), Error<SE, GE>> {
        self.write_data(&value.to_be_bytes())
    }

    /// Writes multiples words with a buffer
    fn write_words_buffered(&mut self, words: impl IntoIterator<Item = u16>) -> Result<(), Error<SE, GE>> 
    {
        let mut buffer = [0; BUFSIZE];
        let mut index = 0;
        for word in words {
            let as_bytes = word.to_be_bytes();
            buffer[index] = as_bytes[0];
            buffer[index + 1] = as_bytes[1];
            index += 2;
            if index >= buffer.len() {
                self.write_data(&buffer)?;
                index = 0;
            }
        }
        self.write_data(&buffer[0..index])
    }

    /// Set the display orientation
    pub fn set_orientation(&mut self, orientation: &Orientation) -> Result<(), Error<SE, GE>> {
        if self.rgb {
            self.write_command(Instruction::MADCTL, &[*orientation as u8])?;
        } else {
            self.write_command(Instruction::MADCTL, &[*orientation as u8 | 0x08])?;
        }
        Ok(())
    }

    /// Sets the global offset of the displayed image
    pub fn set_offset(&mut self, dx: u16, dy: u16) {
        self.dx = dx;
        self.dy = dy;
    }

    /// Sets the address window for the display.
    pub fn set_address_window(&mut self, sx: u16, sy: u16, ex: u16, ey: u16) -> Result<(), Error<SE, GE>> {
        self.write_command(Instruction::CASET, &[])?;
        self.start_data()?;
        self.write_word(sx + self.dx)?;
        self.write_word(ex + self.dx)?;
        self.write_command(Instruction::RASET, &[])?;
        self.start_data()?;
        self.write_word(sy + self.dy)?;
        self.write_word(ey + self.dy)
    }

    /// Sets a pixel color at the given coords.
    pub fn set_pixel(&mut self, x: u16, y: u16, color: u16) -> Result<(), Error<SE, GE>> {
        self.set_address_window(x, y, x, y)?;
        self.write_command(Instruction::RAMWR, &[])?;
        self.start_data()?;
        self.write_word(color)
    }

    /// Writes pixel colors sequentially into the current drawing window
    pub fn write_pixels<P: IntoIterator<Item = u16>>(&mut self, colors: P) -> Result<(), Error<SE, GE>> {
        self.write_command(Instruction::RAMWR, &[])?;
        self.start_data()?;
        for color in colors {
            self.write_word(color)?;
        }
        Ok(())
    }

    /// Write multiple pixels, trading ram size for speed
    pub fn write_pixels_buffered<P: IntoIterator<Item = u16>>(
        &mut self,
        colors: P,
    ) -> Result<(), Error<SE, GE>> {
        self.write_command(Instruction::RAMWR, &[])?;
        self.start_data()?;
        self.write_words_buffered(colors)
    }

    /// Sets pixel colors at the given drawing window
    pub fn set_pixels<P: IntoIterator<Item = u16>>(
        &mut self,
        sx: u16,
        sy: u16,
        ex: u16,
        ey: u16,
        colors: P,
    ) -> Result<(), Error<SE, GE>> {
        self.set_address_window(sx, sy, ex, ey)?;
        self.write_pixels(colors)
    }

    /// Write multiple pixels to the given drawing window, trading ram size for speed
    pub fn set_pixels_buffered<P: IntoIterator<Item = u16>>(
        &mut self,
        sx: u16,
        sy: u16,
        ex: u16,
        ey: u16,
        colors: P,
    ) -> Result<(), Error<SE, GE>> {
        self.set_address_window(sx, sy, ex, ey)?;
        self.write_pixels_buffered(colors)
    }

    /// Allows adjusting gamma correction on the display.
    /// 
    /// Takes in an array `pos` for positive polarity correction and an array `neg` for negative polarity correction.
    /// 
    /// The following values worked well on an ST7735S test device:
    /// pos: &[0x10, 0x0E, 0x02, 0x03, 0x0E, 0x07, 0x02, 0x07, 0x0A, 0x12, 0x27, 0x37, 0x00, 0x0D, 0x0E, 0x10]
    /// neg: &[0x10, 0x0E, 0x03, 0x03, 0x0F, 0x06, 0x02, 0x08, 0x0A, 0x13, 0x26, 0x36, 0x00, 0x0D, 0x0E, 0x10]
    pub fn adjust_gamma(&mut self, pos: &[u8;16], neg: &[u8;16]) -> Result<(), Error<SE, GE>> {
        self.write_command(Instruction::GMCTRP1, pos)?;
        self.write_command(Instruction::GMCTRN1, neg)
    }
}

#[cfg(feature = "graphics")]
extern crate embedded_graphics_core;
#[cfg(feature = "graphics")]
use self::embedded_graphics_core::{
    draw_target::DrawTarget,
    pixelcolor::{
        raw::{RawData, RawU16},
        Rgb565,
    },
    prelude::*,
    primitives::Rectangle,
};

#[cfg(feature = "graphics")]
impl<SE, GE, SPI, DC, RST, const BUFSIZE: usize> DrawTarget for ST7735<SPI, DC, RST, BUFSIZE>
where
    SPI: spi::SpiDevice<Error = SE>,
    DC: OutputPin<Error = GE>,
    RST: OutputPin<Error = GE>,
    SE: embedded_hal::spi::Error,
    GE: embedded_hal::digital::Error,
{
    type Error = crate::Error<SE, GE>;
    type Color = Rgb565;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            // Only draw pixels that would be on screen
            if coord.x >= 0
                && coord.y >= 0
                && coord.x < self.width as i32
                && coord.y < self.height as i32
            {
                self.set_pixel(
                    coord.x as u16,
                    coord.y as u16,
                    RawU16::from(color).into_inner(),
                )?;
            }
        }

        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        // Clamp area to drawable part of the display target
        let drawable_area = area.intersection(&Rectangle::new(Point::zero(), self.size()));

        if drawable_area.size != Size::zero() {
            self.set_pixels_buffered(
                drawable_area.top_left.x as u16,
                drawable_area.top_left.y as u16,
                (drawable_area.top_left.x + (drawable_area.size.width - 1) as i32) as u16,
                (drawable_area.top_left.y + (drawable_area.size.height - 1) as i32) as u16,
                area.points()
                    .zip(colors)
                    .filter(|(pos, _color)| drawable_area.contains(*pos))
                    .map(|(_pos, color)| RawU16::from(color).into_inner()),
            )?;
        }

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.set_pixels_buffered(
            0,
            0,
            self.width as u16 - 1,
            self.height as u16 - 1,
            core::iter::repeat(RawU16::from(color).into_inner())
                .take((self.width * self.height) as usize),
        )
    }
}

#[cfg(feature = "graphics")]
impl<SPI, DC, RST, const BUFSIZE: usize> OriginDimensions for ST7735<SPI, DC, RST, BUFSIZE>
where
    SPI: spi::SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
{
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}
