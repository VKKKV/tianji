use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub bg: Color,
    pub panel_bg: Color,
    pub border: Color,
    pub fg: Color,
    pub label: Color,
    pub value: Color,
    pub up: Color,
    pub down: Color,
    pub warn: Color,
    pub status_bg: Color,
    pub key_hint: Color,
    pub title: Color,
}

pub const KANAGAWA: Theme = Theme {
    bg: Color::Rgb(0x1F, 0x1F, 0x28),
    panel_bg: Color::Rgb(0x27, 0x27, 0x27),
    border: Color::Rgb(0x36, 0x36, 0x46),
    fg: Color::Rgb(0xDC, 0xD7, 0xBA),
    label: Color::Rgb(0x7E, 0x9C, 0xD8),
    value: Color::Rgb(0xDC, 0xD7, 0xBA),
    up: Color::Rgb(0x98, 0xBB, 0x6C),
    down: Color::Rgb(0xE4, 0x68, 0x76),
    warn: Color::Rgb(0xFF, 0xA0, 0x66),
    status_bg: Color::Rgb(0x36, 0x36, 0x46),
    key_hint: Color::Rgb(0x93, 0x8A, 0xA9),
    title: Color::Rgb(0xE6, 0xC3, 0x84),
};
