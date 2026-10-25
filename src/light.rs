pub const HALF_PERIOD_MS: u64 = 500;
pub const COLORS: [(&str, [u8; 3]); 4] = [
    ("red", [10, 0, 0]),
    ("green", [0, 10, 0]),
    ("blue", [0, 0, 10]),
    ("cyan", [0, 10, 10]),
];
pub const OFF: (&str, [u8; 3]) = ("off", [0, 0, 0]);

#[derive(Default)]
pub struct BlinkSequence {
    phase: usize,
}

impl BlinkSequence {
    pub fn next_frame(&mut self, enabled: bool) -> (&'static str, [u8; 3]) {
        if !enabled {
            self.phase = 0;
            return OFF;
        }
        let phase = self.phase;
        self.phase = (self.phase + 1) % (COLORS.len() * 2);
        if phase.is_multiple_of(2) {
            COLORS[phase / 2]
        } else {
            OFF
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LightSettings {
    pub enabled: bool,
    // Matter StartUpOnOff: 0 = off, 1 = on, 2 = toggle, None = restore the previous state.
    pub startup: Option<u8>,
}

impl LightSettings {
    pub fn encode(self) -> [u8; 3] {
        [1, u8::from(self.enabled), self.startup.unwrap_or(255)]
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [1, enabled @ 0..=1, startup @ (0..=2 | 255)] => Some(Self {
                enabled: *enabled == 1,
                startup: if *startup == 255 {
                    None
                } else {
                    Some(*startup)
                },
            }),
            _ => None,
        }
    }
}

pub fn device_identity(mac: [u8; 6]) -> ([u8; 8], [u8; 20], u16) {
    let eui64 = [
        mac[0] | 0x02,
        mac[1],
        mac[2],
        0xff,
        0xfe,
        mac[3],
        mac[4],
        mac[5],
    ];
    let mut name = *b"ESP32C6-000000000000";
    let hex = b"0123456789ABCDEF";
    for (index, byte) in mac.into_iter().enumerate() {
        name[8 + index * 2] = hex[usize::from(byte >> 4)];
        name[9 + index * 2] = hex[usize::from(byte & 15)];
    }
    let discriminator = u16::from_be_bytes([mac[4], mac[5]]) & 0x0fff;
    (eui64, name, discriminator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_four_colors_with_black_between_each() {
        let mut sequence = BlinkSequence::default();
        let expected = [
            ("red", [10, 0, 0]),
            ("green", [0, 10, 0]),
            ("blue", [0, 0, 10]),
            ("cyan", [0, 10, 10]),
        ];
        assert_eq!(COLORS, expected);
        for _ in 0..3 {
            for color in expected {
                assert_eq!(sequence.next_frame(true), color);
                assert_eq!(sequence.next_frame(true), OFF);
            }
        }
        assert_eq!(HALF_PERIOD_MS, 500);
    }

    #[test]
    fn disabling_at_any_phase_is_black_and_restarts_at_red() {
        for phase in 0..COLORS.len() * 2 {
            let mut sequence = BlinkSequence::default();
            for _ in 0..=phase {
                sequence.next_frame(true);
            }
            assert_eq!(sequence.next_frame(false), OFF);
            assert_eq!(sequence.next_frame(false), OFF);
            assert_eq!(sequence.next_frame(true), COLORS[0]);
        }
    }

    #[test]
    fn settings_round_trip_all_valid_values() {
        for enabled in [false, true] {
            for startup in [None, Some(0), Some(1), Some(2)] {
                let settings = LightSettings { enabled, startup };
                assert_eq!(LightSettings::decode(&settings.encode()), Some(settings));
            }
        }
    }

    #[test]
    fn rejects_corrupt_or_unknown_settings() {
        for bytes in [
            &[][..],
            &[1, 0],
            &[1, 0, 255, 0],
            &[2, 0, 255],
            &[1, 2, 255],
            &[1, 0, 3],
        ] {
            assert_eq!(LightSettings::decode(bytes), None);
        }
    }

    #[test]
    fn identity_is_stable_and_board_specific() {
        let mac = [0x40, 0x11, 0x22, 0x33, 0x44, 0x55];
        let identity = device_identity(mac);
        assert_eq!(identity, device_identity(mac));
        assert_eq!(identity.0, [0x42, 0x11, 0x22, 0xff, 0xfe, 0x33, 0x44, 0x55]);
        assert_eq!(&identity.1, b"ESP32C6-401122334455");
        assert!(identity.2 <= 4095);
        assert_ne!(
            identity,
            device_identity([0x40, 0x11, 0x22, 0x33, 0x44, 0x56])
        );
    }
}
