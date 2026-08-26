//! Stable player-input wire contract and deterministic input recordings.

pub mod recording;

/// Exact ten-byte player input payload consumed by the simulation.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerCommand {
    pub opcode: i8,
    pub argument_1: i8,
    pub argument_2: i8,
    pub roll: i8,
    pub pitch: i8,
    pub flags: i8,
    pub argument_6: u16,
    pub argument_8: u16,
}

const _: () = assert!(size_of::<PlayerCommand>() == 10);

impl PlayerCommand {
    #[must_use]
    pub fn to_bytes(self) -> [u8; 10] {
        let argument_6 = self.argument_6.to_le_bytes();
        let argument_8 = self.argument_8.to_le_bytes();
        [
            self.opcode.cast_unsigned(),
            self.argument_1.cast_unsigned(),
            self.argument_2.cast_unsigned(),
            self.roll.cast_unsigned(),
            self.pitch.cast_unsigned(),
            self.flags.cast_unsigned(),
            argument_6[0],
            argument_6[1],
            argument_8[0],
            argument_8[1],
        ]
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 10]) -> Self {
        Self {
            opcode: bytes[0].cast_signed(),
            argument_1: bytes[1].cast_signed(),
            argument_2: bytes[2].cast_signed(),
            roll: bytes[3].cast_signed(),
            pitch: bytes[4].cast_signed(),
            flags: bytes[5].cast_signed(),
            argument_6: u16::from_le_bytes([bytes[6], bytes[7]]),
            argument_8: u16::from_le_bytes([bytes[8], bytes[9]]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_layout_and_bytes_are_exact() {
        assert_eq!(size_of::<PlayerCommand>(), 10);
        let command = PlayerCommand {
            opcode: -1,
            argument_1: 2,
            argument_2: -3,
            roll: 4,
            pitch: -5,
            flags: -128,
            argument_6: 0x1234,
            argument_8: 0xabcd,
        };
        assert_eq!(PlayerCommand::from_bytes(command.to_bytes()), command);
    }
}
