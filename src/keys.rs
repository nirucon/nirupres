//! Keysyms: characters, dead keys and the named keys NIRUPRES uses.

pub fn keysym_char(sym: u32) -> Option<char> {
    match sym {
        0x20..=0x7e | 0xa0..=0xff => char::from_u32(sym),
        0xffb0..=0xffb9 => char::from_u32(sym - 0xffb0 + b'0' as u32),
        0xffaa => Some('*'),
        0xffab => Some('+'),
        0xffad => Some('-'),
        0xffae => Some('.'),
        0xffaf => Some('/'),
        0xffac => Some(','),
        0x20ac => Some('€'),
        0x0100_0000..=0x0110_ffff => char::from_u32(sym - 0x0100_0000),
        _ => None,
    }
}

/// The character a dead key gives on its own (dead key + space).
fn dead_alone(sym: u32) -> Option<char> {
    Some(match sym {
        0xfe50 => '`',
        0xfe51 => '´',
        0xfe52 => '^',
        0xfe53 => '~',
        0xfe57 => '¨',
        _ => return None,
    })
}

/// A dead key followed by a letter.
pub fn compose(dead: u32, c: char) -> Option<char> {
    if c == ' ' {
        return dead_alone(dead);
    }
    let table: &[(char, char)] = match dead {
        0xfe50 => &[
            ('a', 'à'),
            ('e', 'è'),
            ('i', 'ì'),
            ('o', 'ò'),
            ('u', 'ù'),
            ('A', 'À'),
            ('E', 'È'),
            ('O', 'Ò'),
            ('U', 'Ù'),
        ],
        0xfe51 => &[
            ('a', 'á'),
            ('e', 'é'),
            ('i', 'í'),
            ('o', 'ó'),
            ('u', 'ú'),
            ('y', 'ý'),
            ('A', 'Á'),
            ('E', 'É'),
            ('I', 'Í'),
            ('O', 'Ó'),
            ('U', 'Ú'),
        ],
        0xfe52 => &[
            ('a', 'â'),
            ('e', 'ê'),
            ('i', 'î'),
            ('o', 'ô'),
            ('u', 'û'),
            ('A', 'Â'),
            ('E', 'Ê'),
            ('O', 'Ô'),
        ],
        0xfe53 => &[
            ('a', 'ã'),
            ('n', 'ñ'),
            ('o', 'õ'),
            ('A', 'Ã'),
            ('N', 'Ñ'),
            ('O', 'Õ'),
        ],
        0xfe57 => &[
            ('a', 'ä'),
            ('e', 'ë'),
            ('i', 'ï'),
            ('o', 'ö'),
            ('u', 'ü'),
            ('y', 'ÿ'),
            ('A', 'Ä'),
            ('E', 'Ë'),
            ('O', 'Ö'),
            ('U', 'Ü'),
        ],
        _ => &[],
    };
    table.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}

pub const K_ESCAPE: u32 = 0xff1b;
pub const K_RETURN: u32 = 0xff0d;
pub const K_KP_ENTER: u32 = 0xff8d;
pub const K_TAB: u32 = 0xff09;
pub const K_ISO_LEFT_TAB: u32 = 0xfe20;
pub const K_BACKSPACE: u32 = 0xff08;
pub const K_DELETE: u32 = 0xffff;
pub const K_HOME: u32 = 0xff50;
pub const K_LEFT: u32 = 0xff51;
pub const K_UP: u32 = 0xff52;
pub const K_RIGHT: u32 = 0xff53;
pub const K_DOWN: u32 = 0xff54;
pub const K_PRIOR: u32 = 0xff55;
pub const K_NEXT: u32 = 0xff56;
pub const K_END: u32 = 0xff57;
pub const K_F5: u32 = 0xffc2;
pub const K_KP_HOME: u32 = 0xff95;
pub const K_KP_LEFT: u32 = 0xff96;
pub const K_KP_UP: u32 = 0xff97;
pub const K_KP_RIGHT: u32 = 0xff98;
pub const K_KP_DOWN: u32 = 0xff99;
pub const K_KP_PRIOR: u32 = 0xff9a;
pub const K_KP_NEXT: u32 = 0xff9b;
pub const K_KP_END: u32 = 0xff9c;
pub const K_KP_DELETE: u32 = 0xff9f;

/// Keypad navigation keys mapped to the main ones.
pub fn normalize(sym: u32) -> u32 {
    match sym {
        K_KP_HOME => K_HOME,
        K_KP_LEFT => K_LEFT,
        K_KP_UP => K_UP,
        K_KP_RIGHT => K_RIGHT,
        K_KP_DOWN => K_DOWN,
        K_KP_PRIOR => K_PRIOR,
        K_KP_NEXT => K_NEXT,
        K_KP_END => K_END,
        K_KP_DELETE => K_DELETE,
        K_KP_ENTER => K_RETURN,
        s => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_and_dead_keys() {
        assert_eq!(keysym_char(0xe5), Some('å'));
        assert_eq!(keysym_char(0xffb3), Some('3'));
        assert_eq!(compose(0xfe51, 'e'), Some('é'));
        assert_eq!(normalize(K_KP_ENTER), K_RETURN);
    }
}
