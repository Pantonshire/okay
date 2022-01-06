use std::error;
use std::fmt;

use crate::hex::HexBytes;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Header {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) channels: Channels,
    pub(crate) col_space: ColSpace,
}

impl Header {
    pub(crate) const MAGIC: [u8; 4] = *b"qoif";

    pub const fn new(width: u32, height: u32, channels: Channels, col_space: ColSpace) -> Self {
        Self {
            width,
            height,
            channels,
            col_space,
        }
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width(), self.height())
    }

    pub const fn channels(&self) -> Channels {
        self.channels
    }

    pub const fn col_space(&self) -> ColSpace {
        self.col_space
    }

    pub(crate) fn validate_magic(magic: [u8; 4]) -> Result<(), MagicError> {
        if magic == Self::MAGIC {
            Ok(())
        } else {
            Err(MagicError { bad_value: magic })
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Channels {
    Rgb,
    Rgba,
}

impl Channels {
    const RGB_BYTE: u8 = 0x03;
    const RGBA_BYTE: u8 = 0x04;
}

impl TryFrom<u8> for Channels {
    type Error = ChannelsError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            Self::RGB_BYTE => Ok(Channels::Rgb),
            Self::RGBA_BYTE => Ok(Channels::Rgba),
            n => Err(ChannelsError { bad_value: n }),
        }
    }
}

impl From<Channels> for u8 {
    fn from(channels: Channels) -> Self {
        match channels {
            Channels::Rgb => Channels::RGB_BYTE,
            Channels::Rgba => Channels::RGBA_BYTE,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ColSpace {
    Srgb,
    Linear,
}

impl ColSpace {
    const SRGB_BYTE: u8 = 0x00;
    const LINEAR_BYTE: u8 = 0x01;
}

impl TryFrom<u8> for ColSpace {
    type Error = ColSpaceError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            Self::SRGB_BYTE => Ok(ColSpace::Srgb),
            Self::LINEAR_BYTE => Ok(ColSpace::Linear),
            n => Err(ColSpaceError { bad_value: n }),
        }
    }
}

impl From<ColSpace> for u8 {
    fn from(col_space: ColSpace) -> Self {
        match col_space {
            ColSpace::Srgb => ColSpace::SRGB_BYTE,
            ColSpace::Linear => ColSpace::LINEAR_BYTE,
        }
    }
}

#[derive(Debug)]
pub struct MagicError {
    bad_value: [u8; 4],
}

impl MagicError {
    pub fn bad_value(&self) -> [u8; 4] {
        self.bad_value
    }
}

impl fmt::Display for MagicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bad magic bytes {}, expected {}",
            HexBytes::new(&self.bad_value),
            HexBytes::new(&Header::MAGIC)
        )
    }
}

impl error::Error for MagicError {}

#[derive(Debug)]
pub struct ChannelsError {
    bad_value: u8,
}

impl ChannelsError {
    pub fn bad_value(&self) -> u8 {
        self.bad_value
    }
}

impl fmt::Display for ChannelsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid channels value {}, expected {} or {}",
            self.bad_value,
            Channels::RGB_BYTE,
            Channels::RGBA_BYTE
        )
    }
}

impl error::Error for ChannelsError {}

#[derive(Debug)]
pub struct ColSpaceError {
    bad_value: u8,
}

impl ColSpaceError {
    pub fn bad_value(&self) -> u8 {
        self.bad_value
    }
}

impl fmt::Display for ColSpaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid color space value {}, expected {} or {}",
            self.bad_value,
            ColSpace::SRGB_BYTE,
            ColSpace::LINEAR_BYTE
        )
    }
}

impl error::Error for ColSpaceError {}
