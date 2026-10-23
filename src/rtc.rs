//! Bounded Ap µPD4990A calendar/read protocol (NEC IC-1755, pp. 4–8, 11–13).
//! No host clock, time setting, TP/IRQ controls, or test mode.
const CLOCK_HZ: u64 = 8_000_000;

/// Counter seed in binary; year is the chip's two-digit year, weekday Sunday=0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calendar {
    year: u8,
    month: u8,
    day: u8,
    weekday: u8,
    hour: u8,
    minute: u8,
    second: u8,
}
impl Calendar {
    pub fn new(
        year: u8,
        month: u8,
        day: u8,
        weekday: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> Result<Self, &'static str> {
        let date = Self {
            year,
            month,
            day,
            weekday,
            hour,
            minute,
            second,
        };
        if year > 99
            || !(1..=12).contains(&month)
            || day == 0
            || day > date.month_days(true)
            || weekday > 6
            || hour > 23
            || minute > 59
            || second > 59
        {
            return Err("invalid RTC calendar seed");
        }
        Ok(date)
    }
    fn month_days(self, extended: bool) -> u8 {
        match self.month {
            4 | 6 | 9 | 11 => 30,
            2 if extended && self.year.is_multiple_of(4) => 29,
            2 => 28,
            _ => 31,
        }
    }
    fn next_second(&mut self, extended: bool) {
        self.second += 1;
        if self.second < 60 {
            return;
        }
        self.second = 0;
        self.minute += 1;
        if self.minute < 60 {
            return;
        }
        self.minute = 0;
        self.hour += 1;
        if self.hour < 24 {
            return;
        }
        self.hour = 0;
        self.weekday = (self.weekday + 1) % 7;
        self.day += 1;
        if self.day <= self.month_days(extended) {
            return;
        }
        self.day = 1;
        self.month += 1;
        if self.month <= 12 {
            return;
        }
        self.month = 1;
        if extended {
            self.year = (self.year + 1) % 100;
        }
    }
    /// PC-98 INT 1Ch/AH=00 layout, captured in one call.
    pub fn snapshot(self) -> [u8; 6] {
        fn bcd(n: u8) -> u8 {
            ((n / 10) << 4) | (n % 10)
        }
        [
            bcd(self.year),
            self.month << 4 | self.weekday,
            bcd(self.day),
            bcd(self.hour),
            bcd(self.minute),
            bcd(self.second),
        ]
    }
}
impl Default for Calendar {
    fn default() -> Self {
        Self::new(26, 1, 1, 4, 0, 0, 0).unwrap()
    }
}

/// Battery-backed state, including the serial interface, survives machine reset.
/// Construction is the explicit deterministic initialization mechanism.
#[derive(Clone, Debug)]
pub struct Rtc {
    calendar: Calendar,
    fraction: u64,
    control: u8,
    mode: u8,
    extended: bool,
    data: u64,
    command: u8,
}
impl Default for Rtc {
    fn default() -> Self {
        Self::new(Calendar::default())
    }
}
impl Rtc {
    pub fn new(calendar: Calendar) -> Self {
        Self {
            calendar,
            fraction: 0,
            control: 0,
            mode: 0,
            extended: false,
            data: 0,
            command: 0,
        }
    }
    pub fn snapshot(&self) -> [u8; 6] {
        self.calendar.snapshot()
    }
    pub fn tick(&mut self, clocks: u32) {
        self.fraction += u64::from(clocks);
        let seconds = self.fraction / CLOCK_HZ;
        self.fraction %= CLOCK_HZ;
        for _ in 0..seconds {
            self.calendar.next_second(self.extended);
        }
    }
    pub fn data_out(&self) -> u8 {
        if self.mode == 1 {
            (self.data & 1) as u8
        } else {
            u8::from(self.fraction >= CLOCK_HZ / 2)
        }
    }
    pub fn write(&mut self, value: u8) {
        let rising = value & !self.control;
        self.control = value;
        let serial = value & 7 == 7;
        // Figs. 4–5: shift on CLK rising, stable for CPU trailing-edge sampling.
        // The command register shifts independently even in Hold/Time Read.
        if rising & 16 != 0 {
            let input = u64::from(value & 32 != 0);
            if self.mode == 1 {
                let (width, top) = if self.extended {
                    (48, u64::from(self.command & 1))
                } else {
                    (40, input)
                };
                self.data = ((self.data >> 1) | (top << (width - 1))) & ((1 << width) - 1);
            }
            if serial {
                self.command = (self.command >> 1) | ((input as u8) << 3);
            }
        }
        // Figs. 2–3: STB rising latches; unchanged writes cannot retrigger.
        if rising & 8 != 0 {
            let command = if serial { self.command } else { value & 7 };
            if matches!(command, 0 | 1 | 3) {
                self.extended = serial;
                self.mode = command;
                if command == 3 {
                    let [year, month, day, hour, minute, second] = self.snapshot();
                    self.data = u64::from_le_bytes([
                        second,
                        minute,
                        hour,
                        day,
                        month,
                        if serial { year } else { 0 },
                        0,
                        0,
                    ]);
                }
            }
        }
    }
}
