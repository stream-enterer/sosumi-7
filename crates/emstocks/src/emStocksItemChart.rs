// Port of C++ emStocksItemChart.h / emStocksItemChart.cpp

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emPainter::emPainter;
use emcore::emPainter::{TextAlignment, VAlign};
use emcore::emStroke::emStroke;
use emcore::emTexture::emTexture;

use super::emStocksConfig::emStocksConfig;
use super::emStocksListBox::emStocksListBox;
use super::emStocksRec::{
    AddDaysToDate, AddDaysToDateParts, GetCurrentDate, GetDateDifference, GetDateDifferenceParts,
    GetDaysOfMonth, ParseDate, SharePriceToString, StockRec,
};

/// Port of C++ emStocksItemChart::Price.
#[derive(Debug, Clone, Copy, Default)]
pub struct Price {
    pub valid: bool,
    pub value: f64,
}

impl Price {
    /// Port of C++ Price::Set. Parses a string to a price value.
    pub fn Set(&mut self, s: &str) {
        if s.is_empty() {
            self.valid = false;
            self.value = 0.0;
        } else {
            self.value = s.parse::<f64>().unwrap_or(0.0);
            self.valid = self.value > 0.0 || s.starts_with('0');
        }
    }
}

/// Port of C++ emStocksItemChart.
/// View context (pixels_per_unit, viewed state) is provided by the parent
/// panel via set_view_context() instead of C++ emBorder/emPanel inheritance.
pub struct emStocksItemChart {
    /// C++ `emStocksListBox & ListBox;` member reference
    /// (emStocksItemChart.h). (a)-justified `Rc<RefCell<>>`: shared across
    /// `emStocksFilePanel::Cycle` (owner) and the chart's own `Cycle` (which
    /// reads `GetSelectedDateSignal` + `GetSelectedDate()`).
    pub(crate) list_box: Rc<RefCell<emStocksListBox>>,
    /// C++ `emStocksConfig & Config;` member reference. (a)-justified —
    /// co-borrowed with FilePanel + ItemPanel + ControlPanel; chart's own
    /// `Cycle` reads `GetChangeSignal` from the same `Rc`.
    pub(crate) config: Rc<RefCell<emStocksConfig>>,
    /// D-006 first-Cycle init flag for ListBox.SelectedDate + Config.Change.
    /// Mirrors the Phase A `emStocksControlPanel` pattern. Phase D wires the
    /// actual subscribes inside the gated branch.
    pub(crate) subscribed_init: bool,

    // Data state
    data_up_to_date: bool,

    // Time range
    pub start_date: String,
    pub start_year: i32,
    pub start_month: i32,
    pub start_day: i32,
    pub end_date: String,
    pub total_days: i32,
    pub days_per_price: i32,

    // Price data
    pub owning_shares: bool,
    pub trade_price: Price,
    pub trade_price_text: String,
    pub trade_offset_days: i32,
    pub price_on_selected_date: Price,
    pub price_on_selected_date_text: String,
    pub desired_price: Price,
    pub desired_price_text: String,
    pub prices: Vec<Price>,
    pub min_price: Price,
    pub max_price: Price,

    // Transform
    pub x_offset: f64,
    pub x_factor: f64,
    pub y_offset: f64,
    pub y_factor: f64,
    pub lower_price: f64,
    pub upper_price: f64,

    // Associated stock record index (replaces C++ pointer/listener)
    stock_rec_index: Option<usize>,

    // Selected date from listbox
    pub selected_date: String,

    // View context (set by parent panel before painting)
    pub(crate) pixels_per_unit_x: f64,
    pub(crate) pixels_per_unit_y: f64,
    pub(crate) max_label_height: f64,
    pub(crate) viewed: bool,
}

impl emStocksItemChart {
    /// Port of C++ ctor at `emStocksItemChart.cpp:25-66`. C++ takes
    /// `(ParentArg, name, listBox, config)`; the parent/name scaffolding is
    /// provided by the panel-tree owner site. Subscribes (cpp:64-65) are
    /// deferred to the first `Cycle` per D-006.
    pub fn new(
        list_box: Rc<RefCell<emStocksListBox>>,
        config: Rc<RefCell<emStocksConfig>>,
    ) -> Self {
        Self {
            list_box,
            config,
            subscribed_init: false,
            data_up_to_date: false,
            start_date: String::new(),
            start_year: 0,
            start_month: 0,
            start_day: 0,
            end_date: String::new(),
            total_days: 1,
            days_per_price: 1,
            owning_shares: false,
            trade_price: Price {
                valid: false,
                value: 0.0,
            },
            trade_price_text: String::new(),
            trade_offset_days: i32::MIN,
            price_on_selected_date: Price {
                valid: false,
                value: 0.0,
            },
            price_on_selected_date_text: String::new(),
            desired_price: Price {
                valid: false,
                value: 0.0,
            },
            desired_price_text: String::new(),
            prices: Vec::new(),
            min_price: Price {
                valid: false,
                value: 0.0,
            },
            max_price: Price {
                valid: false,
                value: 0.0,
            },
            x_offset: 0.0,
            x_factor: 1.0,
            y_offset: 0.0,
            y_factor: -1.0,
            lower_price: 0.0,
            upper_price: 1.0,
            stock_rec_index: None,
            selected_date: String::new(),
            pixels_per_unit_x: 800.0,
            pixels_per_unit_y: 400.0,
            max_label_height: 0.032,
            viewed: false,
        }
    }

    /// Get the stock rec index.
    pub fn GetStockRecIndex(&self) -> Option<usize> {
        self.stock_rec_index
    }

    /// Set which stock to display.
    pub fn SetStockRecIndex(&mut self, index: Option<usize>) {
        if self.stock_rec_index != index {
            self.stock_rec_index = index;
            self.InvalidateData();
        }
    }

    /// Set the selected date (from ListBox).
    pub fn SetSelectedDate(&mut self, date: &str) {
        if self.selected_date != date {
            self.selected_date = date.to_string();
            self.InvalidateData();
        }
    }

    pub(crate) fn ViewToPanelDeltaX(&self, pixels: f64) -> f64 {
        pixels / self.pixels_per_unit_x
    }

    pub(crate) fn ViewToPanelDeltaY(&self, pixels: f64) -> f64 {
        pixels / self.pixels_per_unit_y
    }

    pub(crate) fn PanelToViewDeltaY(&self, panel_dist: f64) -> f64 {
        panel_dist * self.pixels_per_unit_y
    }

    /// Set view context values. Called by the parent panel before painting.
    pub fn set_view_context(
        &mut self,
        pixels_per_unit_x: f64,
        pixels_per_unit_y: f64,
        viewed: bool,
    ) {
        self.pixels_per_unit_x = pixels_per_unit_x;
        self.pixels_per_unit_y = pixels_per_unit_y;
        self.max_label_height = self.ViewToPanelDeltaY(14.0).min(0.032);
        self.viewed = viewed;
    }

    /// Mark data as needing update.
    pub fn InvalidateData(&mut self) {
        self.data_up_to_date = false;
    }

    /// Port of C++ UpdateData. Recalculates all derived data from StockRec and Config.
    /// Takes stock_rec and config as parameters (avoids needing Rc<RefCell<>> references).
    pub fn UpdateData(&mut self, stock_rec: Option<&StockRec>, config: &emStocksConfig) {
        if self.data_up_to_date {
            return;
        }

        if let Some(rec) = stock_rec {
            self.UpdateTimeRange(rec, config);
            self.UpdatePrices1(rec);
            self.UpdatePrices2(rec);
            self.UpdateTransformation(0.0, 0.0, 1.0, 1.0);
        } else {
            // No stock rec: clear everything
            self.owning_shares = false;
            self.trade_price.valid = false;
            self.trade_price_text.clear();
            self.trade_offset_days = i32::MIN;
            self.price_on_selected_date.valid = false;
            self.price_on_selected_date_text.clear();
            self.desired_price.valid = false;
            self.desired_price_text.clear();
            self.min_price.valid = false;
            self.max_price.valid = false;
            self.prices.clear();
        }

        self.data_up_to_date = true;
    }

    /// Port of C++ UpdateTimeRange.
    fn UpdateTimeRange(&mut self, _stock_rec: &StockRec, config: &emStocksConfig) {
        // C++: EndDate=ListBox.GetSelectedDate();
        self.end_date = self.selected_date.clone();
        if ParseDate(&self.end_date).is_none() {
            self.end_date = GetCurrentDate();
        }
        // C++: EndDate=emStocksFileModel::AddDaysToDate(1,EndDate);
        self.end_date = AddDaysToDate(1, &self.end_date);
        // C++: TotalDays=Config.CalculateChartPeriodDays(EndDate);
        self.total_days = config.CalculateChartPeriodDays(&self.end_date);
        // C++: StartDate=emStocksFileModel::AddDaysToDate(-TotalDays,EndDate);
        self.start_date = AddDaysToDate(-self.total_days, &self.end_date);
        // C++: emStocksRec::ParseDate(StartDate,&StartYear,&StartMonth,&StartDay);
        if let Some((y, m, d)) = ParseDate(&self.start_date) {
            self.start_year = y;
            self.start_month = m;
            self.start_day = d;
        } else {
            self.start_year = 0;
            self.start_month = 0;
            self.start_day = 0;
        }
        // C++: DaysPerPrice=CalculateDaysPerPrice();
        self.days_per_price = self.CalculateDaysPerPrice();
    }

    /// Port of C++ CalculateDaysPerPrice.
    fn CalculateDaysPerPrice(&self) -> i32 {
        if !self.viewed {
            return self.total_days;
        }
        let m = self.total_days / 2;
        let mut d = 1;
        while d < m {
            d <<= 1;
        }
        d /= 256;
        if d <= 0 {
            d = 1;
        }
        d
    }

    /// Port of C++ UpdatePrices1. Sets trade price, price on selected date,
    /// desired price, and initializes min/max from those.
    fn UpdatePrices1(&mut self, stock_rec: &StockRec) {
        self.owning_shares = stock_rec.owning_shares;

        self.trade_price.Set(&stock_rec.trade_price);
        self.min_price = self.trade_price;
        self.max_price = self.trade_price;

        if self.trade_price.valid {
            let label = if self.owning_shares {
                "Purchase Price"
            } else {
                "Sale Price"
            };
            self.trade_price_text = format!("{}: {}", label, &stock_rec.trade_price);

            if !stock_rec.trade_date.is_empty() {
                let (diff, _valid) = GetDateDifference(&self.start_date, &stock_rec.trade_date);
                self.trade_offset_days = diff;
            } else {
                self.trade_offset_days = i32::MIN;
            }
        } else {
            self.trade_price_text.clear();
            self.trade_offset_days = i32::MIN;
        }

        let price_str = stock_rec.GetPriceOfDate(&self.selected_date);
        self.price_on_selected_date.Set(&price_str);
        if self.price_on_selected_date.valid {
            if !self.min_price.valid || self.min_price.value > self.price_on_selected_date.value {
                self.min_price = self.price_on_selected_date;
            }
            if !self.max_price.valid || self.max_price.value < self.price_on_selected_date.value {
                self.max_price = self.price_on_selected_date;
            }
            self.price_on_selected_date_text = format!("Price: {}", price_str);
        } else {
            self.price_on_selected_date_text.clear();
        }

        self.desired_price.Set(&stock_rec.desired_price);
        if self.desired_price.valid {
            if !self.min_price.valid || self.min_price.value > self.desired_price.value {
                self.min_price = self.desired_price;
            }
            if !self.max_price.valid || self.max_price.value < self.desired_price.value {
                self.max_price = self.desired_price;
            }
            self.desired_price_text = format!("Desired Price: {}", &stock_rec.desired_price);
        } else {
            self.desired_price_text.clear();
        }
    }

    /// Port of C++ UpdatePrices2. Populates the prices array from StockRec price
    /// history, computing per-bucket averages and updating min/max.
    fn UpdatePrices2(&mut self, stock_rec: &StockRec) {
        if stock_rec.prices.is_empty() || stock_rec.last_price_date.is_empty() {
            self.prices.clear();
            return;
        }

        let s_bytes = stock_rec.prices.as_bytes();
        let s_len = s_bytes.len();

        let price_count = (self.total_days + self.days_per_price - 1) / self.days_per_price;
        self.prices = vec![
            Price {
                valid: false,
                value: 0.0,
            };
            price_count as usize
        ];

        let mut remaining_days = (self.total_days - 1) % self.days_per_price + 1;

        let (diff_days, _) = GetDateDifference(&stock_rec.last_price_date, &self.end_date);
        let mut diff_days = diff_days - 1;

        // s2 is the exclusive end pointer into the prices string
        let mut s2 = s_len;
        // t2 is the exclusive end index into the prices vec
        let mut t2 = self.prices.len();

        if diff_days < 0 {
            // LastPriceDate is after EndDate: skip prices from the end
            while s2 > 0 {
                s2 -= 1;
                if s_bytes[s2] == b'|' {
                    diff_days += 1;
                    if diff_days >= 0 {
                        break;
                    }
                }
            }
        } else if diff_days > 0 {
            // LastPriceDate is before EndDate: skip buckets from the end
            t2 = t2.saturating_sub((diff_days / self.days_per_price) as usize);
            remaining_days -= diff_days % self.days_per_price;
            if remaining_days <= 0 {
                t2 = t2.saturating_sub(1);
                remaining_days += self.days_per_price;
            }
        }

        if s2 == 0 || t2 == 0 {
            return;
        }

        let mut minv: f64 = 1e100;
        let mut maxv: f64 = -1e100;
        let mut tv: f64 = 0.0;
        let mut n: i32 = 0;

        loop {
            s2 -= 1;
            if s_bytes[s2] != b'|' {
                // Find start of this price value (scan back to previous '|' or start)
                while s2 > 0 && s_bytes[s2 - 1] != b'|' {
                    s2 -= 1;
                }
                // Parse the price value
                let price_str = std::str::from_utf8(&s_bytes[s2..]).unwrap_or("0");
                // Find end of this value (up to next '|' or end)
                let val_end = price_str.find('|').unwrap_or(price_str.len());
                let sv: f64 = price_str[..val_end].parse().unwrap_or(0.0);
                tv += sv;
                n += 1;
                if minv > sv {
                    minv = sv;
                }
                if maxv < sv {
                    maxv = sv;
                }
            }
            remaining_days -= 1;
            if remaining_days <= 0 {
                t2 -= 1;
                if n > 0 {
                    self.prices[t2].valid = true;
                    self.prices[t2].value = tv / n as f64;
                }
                if t2 == 0 {
                    break;
                }
                remaining_days = self.days_per_price;
                tv = 0.0;
                n = 0;
            }
            if s2 == 0 {
                break;
            }
        }

        // Handle leftover partial bucket
        if t2 > 0 && n > 0 {
            t2 -= 1;
            self.prices[t2].valid = true;
            self.prices[t2].value = tv / n as f64;
        }

        if minv <= maxv {
            if !self.min_price.valid || self.min_price.value > minv {
                self.min_price.valid = true;
                self.min_price.value = minv;
            }
            if !self.max_price.valid || self.max_price.value < maxv {
                self.max_price.valid = true;
                self.max_price.value = maxv;
            }
        }
    }

    /// Port of C++ UpdateTransformation.
    fn UpdateTransformation(&mut self, cx: f64, cy: f64, cw: f64, ch: f64) {
        let x: f64 = cx;
        let mut y: f64 = cy;
        let w: f64 = cw;
        let mut h: f64 = ch;
        let d = h * 0.008;
        y += d;
        h -= 2.0 * d;

        self.x_offset = x;
        if self.total_days > 0 {
            self.x_factor = w / self.total_days as f64;
        } else {
            self.x_factor = 1.0;
        }

        if self.min_price.valid && self.max_price.valid {
            let c: f64;
            if self.trade_price.valid {
                c = self.trade_price.value;
            } else if self.desired_price.valid {
                c = self.desired_price.value;
            } else {
                c = (self.min_price.value + self.max_price.value) * 0.5;
            }
            let d_price = f64::max(
                0.5 * c,
                f64::max(self.max_price.value - c, c - self.min_price.value),
            );
            let mut p1 = c - d_price;
            let mut p2 = c + d_price;
            if p1 < 0.0 {
                p1 = f64::min(0.0, self.min_price.value);
                p2 = self.max_price.value;
            }
            p2 = f64::max(p2, p1 + 1e-6);

            self.y_factor = h / (p1 - p2);
            self.y_offset = y - self.y_factor * p2;
            self.lower_price = p1;
            self.upper_price = p2;
        } else {
            let p1 = 0.0;
            let p2 = 100.0001;
            self.y_factor = h / (p1 - p2);
            self.y_offset = y - self.y_factor * p2;
            self.lower_price = p1;
            self.upper_price = p2;
        }
    }

    // -----------------------------------------------------------------------
    // Paint pipeline
    // -----------------------------------------------------------------------

    /// Port of C++ PaintContent. Orchestrator that calls all 7 sub-paint methods.
    /// C++ takes (painter, x, y, w, h, canvasColor) from emBorder override.
    /// Rust accepts only painter; view context is stored on the struct via set_view_context().
    pub fn PaintContent(&self, painter: &mut emPainter) {
        self.PaintXScaleLines(painter);
        self.PaintYScaleLines(painter);
        self.PaintXScaleLabels(painter);
        self.PaintYScaleLabels(painter);
        self.PaintPriceBar(painter);
        self.PaintDesiredPrice(painter);
        self.PaintGraph(painter);
    }

    /// Port of C++ PaintXScaleLines. Draws vertical grid lines at day/month/year/decade intervals.
    fn PaintXScaleLines(&self, painter: &mut emPainter) {
        let f = self.ViewToPanelDeltaX(14.0) / self.x_factor;
        let min_level: i32;
        if f <= 1.0 {
            min_level = 0; // days
        } else if f <= 30.4 {
            min_level = 1; // months
        } else if f <= 365.25 {
            min_level = 2; // years
        } else if f <= 3652.5 {
            min_level = 3; // 10 years
        } else {
            return;
        }

        let max_thickness = f64::min(0.002, self.ViewToPanelDeltaX(2.6));

        let f_day = f64::max(
            0.0,
            (painter.GetUserClipX1() - self.x_offset - max_thickness * 0.5) / self.x_factor,
        );
        let f_end_day = f64::min(
            self.total_days as f64,
            (painter.GetUserClipX2() - self.x_offset + max_thickness * 0.5) / self.x_factor,
        );
        if f_day > f_end_day {
            return;
        }
        let mut day = f_day.ceil() as i32;
        let end_day = f_end_day as i32;

        let mut year = self.start_year;
        let mut month = self.start_month;
        let mut mday = self.start_day;
        AddDaysToDateParts(day, &mut year, &mut month, &mut mday);

        if min_level > 0 {
            if mday > 1 {
                day += GetDaysOfMonth(year, month) - mday + 1;
                mday = 1;
                month += 1;
                if month > 12 {
                    year += 1;
                    month = 1;
                }
            }
            if min_level > 1 {
                if month > 1 {
                    day += GetDateDifferenceParts(year, month, 1, year + 1, 1, 1);
                    year += 1;
                    month = 1;
                }
                if min_level > 2 && year % 10 != 0 {
                    let y10 = year + 10 - year % 10;
                    day += GetDateDifferenceParts(year, 1, 1, y10, 1, 1);
                    year = y10;
                }
            }
        }

        let y = self.y_offset + self.y_factor * self.upper_price;
        let h = self.y_factor * (self.lower_price - self.upper_price);
        let c = emColor::rgb(128, 128, 128);

        while day <= end_day {
            let x = self.x_offset + self.x_factor * day as f64;

            let mut t = 0.01;
            if mday == 1 {
                t = 0.01 * 30.4;
                if month == 1 {
                    t = 0.01 * 365.25;
                    if year % 10 == 0 {
                        t = 0.01 * 3652.5;
                    }
                }
            }
            t *= self.x_factor;
            if t > max_thickness {
                t = max_thickness;
            }
            painter.PaintRect(x - t * 0.5, y, t, h, c, emColor::TRANSPARENT);

            if min_level == 0 {
                day += 1;
                mday += 1;
                if mday > GetDaysOfMonth(year, month) {
                    mday = 1;
                    month += 1;
                    if month > 12 {
                        year += 1;
                        month = 1;
                    }
                }
            } else if min_level == 1 {
                day += GetDaysOfMonth(year, month);
                month += 1;
                if month > 12 {
                    year += 1;
                    month = 1;
                }
            } else if min_level == 2 {
                day += 365 - 28 + GetDaysOfMonth(year, 2);
                year += 1;
            } else {
                day += GetDateDifferenceParts(year, 1, 1, year + 10, 1, 1);
                year += 10;
            }
        }
    }

    /// Port of C++ PaintXScaleLabels. Draws date labels below the chart area.
    fn PaintXScaleLabels(&self, painter: &mut emPainter) {
        const MONTH_TEXTS: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];

        let max_text_height = self.max_label_height;
        let min_text_height = self.ViewToPanelDeltaY(6.0);

        let text_width: [f64; 4] = [
            0.8 * self.x_factor,
            27.0 * self.x_factor,
            300.0 * self.x_factor,
            3000.0 * self.x_factor,
        ];
        let text_height: [f64; 4] = [
            f64::min(max_text_height, text_width[0] * 0.8),
            f64::min(max_text_height, text_width[1] * 0.2),
            f64::min(max_text_height, text_width[2] * 0.4),
            f64::min(max_text_height, text_width[3] * 0.4),
        ];

        if text_height[3] < min_text_height {
            return;
        }

        let f_start_day = f64::max(
            0.0,
            (painter.GetUserClipX1() - self.x_offset) / self.x_factor,
        );
        let f_end_day = f64::min(
            self.total_days as f64,
            (painter.GetUserClipX2() - self.x_offset) / self.x_factor,
        );
        if f_start_day >= f_end_day {
            return;
        }
        let start_day = f_start_day as i32;
        let end_day = f_end_day as i32;

        // C++ uses ViewToPanelY(GetClipY2()) for y positioning. Rust uses self.ViewToPanelDeltaY().
        let mut y = f64::max(
            self.y_offset + self.y_factor * self.lower_price,
            self.y_offset + self.y_factor * self.upper_price + 2.5 * max_text_height,
        );
        let c = emColor::rgba(170, 170, 170, 192);

        let mut max_level: i32 = 3;
        if text_height[2] >= 0.9 * text_height[3] {
            max_level = 2;
            if text_height[1] >= 0.9 * text_height[2] && text_width[1] / text_height[1] > 12.0 {
                max_level = 1;
            }
        }

        let mut level = max_level;
        while level >= 0 {
            if text_height[level as usize] < min_text_height {
                break;
            }
            y -= text_height[level as usize];

            let mut year = self.start_year;
            let mut month = self.start_month;
            let mut mday = self.start_day;
            AddDaysToDateParts(start_day, &mut year, &mut month, &mut mday);
            let mut day = start_day;

            if level > 0 {
                if mday > 1 {
                    day -= mday - 1;
                    mday = 1;
                }
                if level > 1 {
                    if month > 1 {
                        day -= GetDateDifferenceParts(year, 1, 1, year, month, 1);
                        month = 1;
                    }
                    if level > 2 && year % 10 != 0 {
                        let y10 = year - year % 10;
                        day -= GetDateDifferenceParts(y10, 1, 1, year, 1, 1);
                        year = y10;
                    }
                }
            }

            while day <= end_day {
                let x1 = self.x_offset + self.x_factor * f64::max(day as f64, f_start_day);

                let label: String;
                if level == 0 {
                    label = format!("{}", mday);
                    day += 1;
                    mday += 1;
                    if mday > GetDaysOfMonth(year, month) {
                        mday = 1;
                        month += 1;
                        if month > 12 {
                            year += 1;
                            month = 1;
                        }
                    }
                } else if level == 1 {
                    if max_level == 1 {
                        label = format!("{} {}", MONTH_TEXTS[(month - 1) as usize], year);
                    } else {
                        label = MONTH_TEXTS[(month - 1) as usize].to_string();
                    }
                    day += GetDaysOfMonth(year, month);
                    month += 1;
                    if month > 12 {
                        year += 1;
                        month = 1;
                    }
                } else if level == 2 {
                    label = format!("{}", year);
                    day += 365 - 28 + GetDaysOfMonth(year, 2);
                    year += 1;
                } else {
                    label = format!("{}x", year / 10);
                    day += GetDateDifferenceParts(year, 1, 1, year + 10, 1, 1);
                    year += 10;
                }

                let x2 = self.x_offset + self.x_factor * f64::min(day as f64, f_end_day);
                if x1 < x2 {
                    let th = text_height[level as usize];
                    painter.PaintTextBoxed(
                        x1,
                        y,
                        x2 - x1,
                        th,
                        &label,
                        th,
                        c,
                        emColor::TRANSPARENT,
                        TextAlignment::Left,
                        VAlign::Top,
                        TextAlignment::Left,
                        0.0,
                        false,
                        0.0,
                    );
                }
            }
            level -= 1;
        }
    }

    /// Port of C++ PaintYScaleLines. Draws horizontal grid lines at price levels.
    fn PaintYScaleLines(&self, painter: &mut emPainter) {
        let (min_level, min_dist, max_level) = self.CalculateYScaleLevelRange();
        if min_level > max_level {
            return;
        }

        let max_thickness = f64::min(0.002, self.ViewToPanelDeltaY(2.6));

        let mut price = f64::max(
            self.lower_price,
            (painter.GetUserClipY2() - self.y_offset + max_thickness * 0.5) / self.y_factor,
        );
        let end_price = f64::min(
            self.upper_price,
            (painter.GetUserClipY1() - self.y_offset - max_thickness * 0.5) / self.y_factor,
        );
        if price > end_price {
            return;
        }

        let f = price % min_dist;
        if f > 0.0 {
            price += min_dist - f;
        } else if f < 0.0 {
            price -= f;
        }

        let x = self.x_offset;
        let w = self.x_factor * self.total_days as f64;
        let c = emColor::rgb(128, 128, 128);

        while price <= end_price {
            let y = self.y_offset + self.y_factor * price;
            let mut level = min_level;
            let mut dist = min_dist;
            while level < max_level {
                let next_dist = dist * if level & 1 != 0 { 2.0 } else { 5.0 };
                let f = price / next_dist;
                if (f - f.round()).abs() > 0.001 {
                    break;
                }
                dist = next_dist;
                level += 1;
            }
            let mut t = dist * self.y_factor * (-0.01);
            if level & 1 != 0 {
                t *= 0.63;
            }
            if t > max_thickness {
                t = max_thickness;
            }
            painter.PaintRect(x, y - t * 0.5, w, t, c, emColor::TRANSPARENT);
            price += min_dist;
        }
    }

    /// Port of C++ PaintYScaleLabels. Draws price labels on the left side.
    fn PaintYScaleLabels(&self, painter: &mut emPainter) {
        let (min_level, min_dist, max_level) = self.CalculateYScaleLevelRange();
        if min_level > max_level {
            return;
        }

        let max_text_height = self.max_label_height;
        let min_text_height = self.ViewToPanelDeltaY(6.0);

        let mut price = f64::max(
            self.lower_price,
            (painter.GetUserClipY2() - self.y_offset + max_text_height) / self.y_factor,
        );
        let end_price = f64::min(
            self.upper_price,
            (painter.GetUserClipY1() - self.y_offset) / self.y_factor,
        );
        if price > end_price {
            return;
        }

        let f = price % min_dist;
        if f > 0.0 {
            price += min_dist - f;
        } else if f < 0.0 {
            price -= f;
        }

        let x = self.x_offset;
        let w = self.x_factor * self.total_days as f64;
        let c = emColor::rgba(170, 170, 170, 192);
        // C++ uses ViewToPanelX(GetClipX1()) for xt. Rust uses x_offset.
        let xt = x;

        while price <= end_price {
            let y = self.y_offset + self.y_factor * price;
            let mut level = min_level;
            let mut dist = min_dist;
            while level < max_level {
                let next_dist = dist * if level & 1 != 0 { 2.0 } else { 5.0 };
                let f = price / next_dist;
                if (f - f.round()).abs() > 0.001 {
                    break;
                }
                dist = next_dist;
                level += 1;
            }
            let mut t = dist * self.y_factor * (-0.16);
            if level & 1 != 0 {
                t *= 0.63;
            }
            if t < min_text_height {
                price += min_dist;
                continue;
            }
            if t > max_text_height {
                t = max_text_height;
            }
            // Format with appropriate decimal places based on level
            let decimals = if level >= 0 {
                0
            } else {
                ((1 - level) >> 1) as usize
            };
            let label = format!("{:.prec$}", price, prec = decimals);
            painter.PaintTextBoxed(
                xt,
                y - t,
                x + w - xt,
                t,
                &label,
                t,
                c,
                emColor::TRANSPARENT,
                TextAlignment::Left,
                VAlign::Top,
                TextAlignment::Left,
                0.0,
                false,
                0.0,
            );
            price += min_dist;
        }
    }

    /// Port of C++ CalculateYScaleLevelRange. Computes price grid spacing
    /// using 1-2-5 progression (logarithmic).
    /// Idiom adaptation: returns (min_level, min_dist, max_level) tuple instead of C++ output
    /// pointers.
    pub(crate) fn CalculateYScaleLevelRange(&self) -> (i32, f64, i32) {
        let mut max_level: i32 = 0;
        let mut max_dist: f64 = 1.0;

        let f = (self.upper_price - self.lower_price) * 0.4;
        while max_dist > f {
            max_level -= 2;
            max_dist *= 0.1;
        }
        while max_dist * 10.0 <= f {
            max_level += 2;
            max_dist *= 10.0;
        }

        let mut min_level = max_level;
        let mut min_dist = max_dist;

        if max_dist * 5.0 <= f {
            max_level += 1;
            max_dist *= 5.0;
        }
        let _ = max_dist; // not used beyond this point

        let f = f64::max(
            f64::max(self.lower_price.abs(), self.upper_price.abs()) * 0.0001,
            self.ViewToPanelDeltaY(14.0) / (-self.y_factor),
        );

        while min_dist < f {
            min_level += 2;
            min_dist *= 10.0;
        }
        while min_dist * 0.1 >= f {
            min_level -= 2;
            min_dist *= 0.1;
        }

        if min_dist * 0.5 >= f {
            min_level -= 1;
            min_dist *= 0.5;
        }

        (min_level, min_dist, max_level)
    }

    /// Port of C++ PaintPriceBar. Draws a colored rectangle between trade/desired price
    /// and current price, with gradient coloring and text labels.
    fn PaintPriceBar(&self, painter: &mut emPainter) {
        if !self.price_on_selected_date.valid
            || (!self.trade_price.valid && !self.desired_price.valid)
        {
            return;
        }

        let text_height = (self.lower_price - self.upper_price) * self.y_factor * 0.012;
        let x = self.x_offset;
        let w = self.x_factor * self.total_days as f64;
        let ref_price = if self.trade_price.valid {
            self.trade_price.value
        } else {
            self.desired_price.value
        };
        let y1 = self.y_offset + self.y_factor * ref_price;
        let y2 = self.y_offset + self.y_factor * self.price_on_selected_date.value;

        let c2 = if self.owning_shares {
            if y1 > y2 {
                emColor::rgba(80, 255, 80, 224)
            } else {
                emColor::rgba(255, 80, 80, 224)
            }
        } else if y1 > y2 {
            emColor::rgba(255, 80, 255, 224)
        } else {
            emColor::rgba(80, 255, 255, 224)
        };
        let c1 = c2.GetBlended(emColor::rgba(128, 128, 255, 224), 50.0);

        let bar_y = f64::min(y1, y2);
        let bar_h = (y2 - y1).abs();
        let gradient = emTexture::LinearGradient {
            color_a: c1.GetTransparented(30.0),
            color_b: c2.GetTransparented(10.0),
            start: (x, y1),
            end: (x, y2),
        };
        painter.paint_polygon_textured(
            &[
                (x, bar_y),
                (x + w, bar_y),
                (x + w, bar_y + bar_h),
                (x, bar_y + bar_h),
            ],
            &gradient,
            emColor::TRANSPARENT,
        );

        if self.PanelToViewDeltaY(text_height) < 4.0 {
            return;
        }

        // Price dot and label
        let xt_base = self.x_offset + self.x_factor * (self.total_days as f64 - 0.5);
        let r = text_height * 0.12;
        painter.PaintEllipse(
            xt_base - r,
            y2 - r,
            r * 2.0,
            r * 2.0,
            c2,
            emColor::TRANSPARENT,
        );

        let (wt, _) =
            emPainter::GetTextSize(&self.price_on_selected_date_text, text_height, false, 0.0);
        let mut xt = xt_base - wt * 0.5;
        let x_right = self.x_offset + self.x_factor * self.total_days as f64 - wt;
        if xt > x_right {
            xt = x_right;
        }
        let label_y = if y1 > y2 { y2 - text_height } else { y2 };
        painter.PaintTextBoxed(
            xt,
            label_y,
            wt,
            text_height,
            &self.price_on_selected_date_text,
            text_height,
            c2,
            emColor::TRANSPARENT,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            0.0,
            false,
            0.0,
        );

        if !self.trade_price.valid {
            return;
        }

        // Trade price dot and label
        let xt_trade: f64;
        if self.trade_offset_days >= 0 {
            xt_trade = self.x_offset + self.x_factor * (self.trade_offset_days as f64 + 0.5);
            if self.trade_offset_days < self.total_days {
                painter.PaintEllipse(
                    xt_trade - r,
                    y1 - r,
                    r * 2.0,
                    r * 2.0,
                    c1,
                    emColor::TRANSPARENT,
                );
            }
        } else if self.trade_offset_days > i32::MIN {
            xt_trade = self.x_offset;
        } else {
            xt_trade = self.x_offset + self.x_factor * self.total_days as f64 * 0.5;
        }

        let (wt, _) = emPainter::GetTextSize(&self.trade_price_text, text_height, false, 0.0);
        let mut xt = xt_trade - wt * 0.5;
        if xt < self.x_offset {
            xt = self.x_offset;
        }
        let x_right = self.x_offset + self.x_factor * self.total_days as f64 - wt;
        if xt > x_right {
            xt = x_right;
        }
        let label_y = if y1 > y2 { y1 } else { y1 - text_height };
        painter.PaintTextBoxed(
            xt,
            label_y,
            wt,
            text_height,
            &self.trade_price_text,
            text_height,
            c1,
            emColor::TRANSPARENT,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            0.0,
            false,
            0.0,
        );
    }

    /// Port of C++ PaintDesiredPrice. Draws a horizontal yellow line at the desired price.
    fn PaintDesiredPrice(&self, painter: &mut emPainter) {
        if !self.desired_price.valid {
            return;
        }

        let thickness = f64::max(
            self.ViewToPanelDeltaY(1.5),
            f64::min(
                (self.lower_price - self.upper_price) * self.y_factor * 0.002,
                self.x_factor * 0.5,
            ),
        );
        let text_height = (self.lower_price - self.upper_price) * self.y_factor * 0.012;
        let x = self.x_offset;
        let w = self.x_factor * self.total_days as f64;
        let y = self.y_offset + self.y_factor * self.desired_price.value - thickness * 0.5;
        let c = emColor::rgba(255, 255, 0, 224);

        painter.PaintRect(x, y, w, thickness, c, emColor::TRANSPARENT);

        if self.PanelToViewDeltaY(text_height) < 4.0 {
            return;
        }

        let v = self.desired_price.value;
        let (mut v1, mut v2);
        if self.price_on_selected_date.valid {
            v1 = self.price_on_selected_date.value;
            v2 = self.price_on_selected_date.value;
            if self.trade_price.valid {
                if self.trade_price.value < v2 {
                    v1 = self.trade_price.value;
                } else {
                    v2 = self.trade_price.value;
                }
            }
        } else {
            v1 = v;
            v2 = v;
        }
        let label_y = if v > v2 || (v >= v1 && v < (v1 + v2) * 0.5) {
            y - text_height
        } else {
            y + thickness
        };

        painter.PaintTextBoxed(
            x,
            label_y,
            w,
            text_height,
            &self.desired_price_text,
            text_height,
            c,
            emColor::TRANSPARENT,
            TextAlignment::Right,
            VAlign::Top,
            TextAlignment::Right,
            0.0,
            false,
            0.0,
        );
    }

    /// Port of C++ PaintGraph. Draws the price line graph with optional point markers
    /// and date/price text labels at high zoom.
    fn PaintGraph(&self, painter: &mut emPainter) {
        if self.prices.len() < 2 {
            return;
        }

        let price_count = self.prices.len() as f64;
        let x_off = self.x_offset + self.x_factor * 0.5;
        let x_fac = self.x_factor * (self.total_days as f64 - 1.0) / (price_count - 1.0);

        let f = (painter.GetUserClipX1() - x_off) / x_fac - 0.5;
        if f >= price_count {
            return;
        }
        let i1: usize = if f < 1.0 { 0 } else { f as usize };
        let f = (painter.GetUserClipX2() - x_off) / x_fac + 0.5;
        if f <= 0.0 {
            return;
        }
        let i2: usize = if f > price_count - 2.0 {
            self.prices.len() - 1
        } else {
            f.ceil() as usize
        };
        if i1 >= i2 {
            return;
        }

        let thickness = f64::max(
            self.ViewToPanelDeltaY(1.5),
            f64::min(
                (self.lower_price - self.upper_price) * self.y_factor * 0.002,
                self.x_factor * 0.1,
            ),
        );
        let r = f64::min(0.002, self.x_factor * 0.1) * 3.0;
        let have_points = self.days_per_price == 1 && r > self.ViewToPanelDeltaY(1.2);
        let have_texts = have_points && r > self.ViewToPanelDeltaY(5.0);

        let c1 = emColor::rgb(255, 255, 255);
        let c2 = emColor::rgb(64, 64, 64);

        // Find first valid price at or before i1
        let mut i0 = i1;
        while i0 > 0 && !self.prices[i0].valid {
            i0 -= 1;
        }
        // Find last valid price at or after i2
        let mut i3 = i2;
        while i3 < self.prices.len() - 1 && !self.prices[i3].valid {
            i3 += 1;
        }

        // C++ uses per-segment PaintLine with emRoundedStroke and emStrokeEnd.
        // PaintPolyline with a simple stroke is the closest equivalent.
        let mut vertices: Vec<(f64, f64)> = Vec::new();
        for i in i0..=i3 {
            if !self.prices[i].valid {
                continue;
            }
            let x2 = x_off + x_fac * i as f64;
            let y2 = self.y_offset + self.y_factor * self.prices[i].value;
            vertices.push((x2, y2));
        }
        if vertices.len() >= 2 {
            let stroke = emStroke::new(c1, thickness);
            painter.PaintPolyline(&vertices, &stroke, false, emColor::TRANSPARENT);
        }

        if !have_points {
            return;
        }

        // Draw point markers
        for i in i1..=i2 {
            if !self.prices[i].valid {
                continue;
            }
            let px = x_off + x_fac * i as f64;
            let py = self.y_offset + self.y_factor * self.prices[i].value;
            painter.PaintEllipse(px - r, py - r, r * 2.0, r * 2.0, c1, emColor::TRANSPARENT);
        }

        if !have_texts {
            return;
        }

        // Draw date and price text labels at each point
        let mut year = self.start_year;
        let mut month = self.start_month;
        let mut mday = self.start_day;
        let mut day: i32 = 0;
        for i in i1..=i2 {
            if !self.prices[i].valid {
                continue;
            }
            let px = x_off + x_fac * i as f64;
            let py = self.y_offset + self.y_factor * self.prices[i].value;
            AddDaysToDateParts(i as i32 - day, &mut year, &mut month, &mut mday);
            day = i as i32;
            let date_str = format!("{:04}-{:02}-{:02}", year, month, mday);
            painter.PaintTextBoxed(
                px - r * 0.8,
                py - r * 0.6,
                r * 0.8 * 2.0,
                r * 0.4,
                &date_str,
                r,
                c2,
                c1,
                TextAlignment::Center,
                VAlign::Center,
                TextAlignment::Center,
                0.0,
                false,
                0.0,
            );
            let price_str = SharePriceToString(self.prices[i].value);
            painter.PaintTextBoxed(
                px - r * 0.8,
                py - r * 0.2,
                r * 0.8 * 2.0,
                r * 0.9,
                &price_str,
                r,
                c2,
                emColor::TRANSPARENT,
                TextAlignment::Center,
                VAlign::Center,
                TextAlignment::Center,
                0.0,
                false,
                0.0,
            );
        }
    }
}

#[cfg(test)]
impl emStocksItemChart {
    /// Phase C test accessor — strong_count probes for the `Rc<RefCell<>>`
    /// member refs so the tests can confirm the ctor wires them through.
    /// Not called from production code; the refs are read by Phase D's
    /// wired `Cycle` body.
    #[doc(hidden)]
    pub(crate) fn list_box_strong_count(&self) -> usize {
        Rc::strong_count(&self.list_box)
    }

    #[doc(hidden)]
    pub(crate) fn config_strong_count(&self) -> usize {
        Rc::strong_count(&self.config)
    }
}

/// Phase C structural scaffold — first-Cycle latch only.
///
/// Phase D will replace the no-op gated body with the rows -64 / -65 D-006
/// subscribes (Config.GetChangeSignal + ListBox.GetSelectedDateSignal). For
/// now the body only flips `subscribed_init` to pin the latch contract.
impl emcore::emPanel::PanelBehavior for emStocksItemChart {
    fn Cycle(
        &mut self,
        _ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        _pctx: &mut emcore::emEngineCtx::PanelCtx,
    ) -> bool {
        if !self.subscribed_init {
            // Phase D will replace these placeholders with the real subscribes
            // (rows -64 / -65). For now we touch the refs so the structural
            // ctor wiring is exercised by the live `Cycle` path rather than
            // only by tests.
            let _ = self.list_box.borrow();
            let _ = self.config.borrow();
            self.subscribed_init = true;
        }
        false
    }
}

#[cfg(test)]
impl emStocksItemChart {
    /// Test-only fixture mirroring Phase A `emStocksControlPanel::for_test`.
    pub(crate) fn for_test() -> Self {
        Self::new(
            Rc::new(RefCell::new(emStocksListBox::new())),
            Rc::new(RefCell::new(emStocksConfig::default())),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emStocksConfig::ChartPeriod;

    #[test]
    fn price_set_valid() {
        let mut p = Price::default();
        p.Set("100.50");
        assert!(p.valid);
        assert!((p.value - 100.5).abs() < 1e-10);
    }

    #[test]
    fn price_set_empty() {
        let mut p = Price::default();
        p.Set("");
        assert!(!p.valid);
    }

    #[test]
    fn price_set_zero() {
        let mut p = Price::default();
        p.Set("0");
        assert!(p.valid);
        assert_eq!(p.value, 0.0);
    }

    #[test]
    fn chart_new_defaults() {
        let chart = emStocksItemChart::for_test();
        assert!(!chart.data_up_to_date);
        assert_eq!(chart.total_days, 1);
        assert_eq!(chart.days_per_price, 1);
    }

    /// B-001-followup C.2 — verify the ctor wires the two member-ref
    /// `Rc<RefCell<>>`s through (strong_count goes to 2 once held both by
    /// the test scope and by the chart). Mirrors the Phase A `holds_member_refs`
    /// shape on `emStocksControlPanel`.
    #[test]
    fn chart_holds_member_refs() {
        let list_box = Rc::new(RefCell::new(emStocksListBox::new()));
        let config = Rc::new(RefCell::new(emStocksConfig::default()));
        let chart = emStocksItemChart::new(list_box.clone(), config.clone());
        assert_eq!(chart.list_box_strong_count(), 2);
        assert_eq!(chart.config_strong_count(), 2);
        assert!(!chart.subscribed_init);
        drop(chart);
        assert_eq!(Rc::strong_count(&list_box), 1);
        assert_eq!(Rc::strong_count(&config), 1);
    }

    /// B-001-followup C.3 — first-Cycle latch flips `subscribed_init`.
    /// Phase D will replace the no-op gated body with the rows -64 / -65
    /// D-006 row subscribes; this test pins the latch contract until then.
    #[test]
    fn chart_first_cycle_flips_subscribed_init() {
        use emcore::emEngine::Priority;
        use emcore::emPanelScope::PanelScope;
        use emcore::test_view_harness::TestViewHarness;

        struct NoopEngine;
        impl emcore::emEngine::emEngine for NoopEngine {
            fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
                false
            }
        }

        let mut h = TestViewHarness::new();
        let eid = h.scheduler.register_engine(
            Box::new(NoopEngine),
            Priority::Medium,
            PanelScope::Framework,
        );

        let mut chart = emStocksItemChart::for_test();
        assert!(!chart.subscribed_init);

        let mut tree = emcore::emPanelTree::PanelTree::new();
        let id = tree.create_root("ic", false);
        {
            let mut pctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, id, 1.0);
            let mut ectx = h.engine_ctx(eid);
            let _ = <emStocksItemChart as emcore::emPanel::PanelBehavior>::Cycle(
                &mut chart, &mut ectx, &mut pctx,
            );
        }

        assert!(
            chart.subscribed_init,
            "first Cycle must flip subscribed_init"
        );

        h.scheduler.remove_engine(eid);
        h.scheduler.flush_signals_for_test();
    }

    #[test]
    fn chart_update_data_no_stock() {
        let mut chart = emStocksItemChart::for_test();
        let config = emStocksConfig::default();
        chart.UpdateData(None, &config);
        assert!(chart.prices.is_empty());
    }

    #[test]
    fn chart_update_data_with_stock() {
        let mut chart = emStocksItemChart::for_test();
        chart.selected_date = "2024-06-15".to_string();
        let config = emStocksConfig {
            chart_period: ChartPeriod::Week1,
            ..Default::default()
        };
        let mut stock = StockRec::default();
        stock.AddPrice("2024-06-10", "100");
        stock.AddPrice("2024-06-15", "105");

        chart.UpdateData(Some(&stock), &config);
        assert!(chart.total_days > 0);
        assert!(!chart.prices.is_empty());
    }

    #[test]
    fn calculate_days_per_price() {
        let mut chart = emStocksItemChart::for_test();
        // viewed=false: returns total_days directly
        chart.total_days = 365;
        assert_eq!(chart.CalculateDaysPerPrice(), 365);

        // viewed=true: uses power-of-2 / 256 algorithm
        chart.viewed = true;
        chart.total_days = 365;
        assert_eq!(chart.CalculateDaysPerPrice(), 1); // 256/256=1, next power 512/256=2 but m=182, d=256 >= 182 so d=256, 256/256=1

        chart.total_days = 7;
        assert_eq!(chart.CalculateDaysPerPrice(), 1); // 4/256 = 0 -> 1
    }

    #[test]
    fn chart_trade_price_text() {
        let mut chart = emStocksItemChart::for_test();
        chart.selected_date = "2024-06-15".to_string();
        let config = emStocksConfig {
            chart_period: ChartPeriod::Week1,
            ..Default::default()
        };
        let mut stock = StockRec::default();
        stock.owning_shares = true;
        stock.trade_price = "50.00".to_string();
        stock.trade_date = "2024-06-12".to_string();
        stock.AddPrice("2024-06-15", "55");

        chart.UpdateData(Some(&stock), &config);
        assert!(chart.trade_price.valid);
        assert!(chart.trade_price_text.contains("Purchase Price"));
    }

    #[test]
    fn chart_desired_price() {
        let mut chart = emStocksItemChart::for_test();
        chart.selected_date = "2024-06-15".to_string();
        let config = emStocksConfig {
            chart_period: ChartPeriod::Week1,
            ..Default::default()
        };
        let mut stock = StockRec::default();
        stock.desired_price = "120.00".to_string();
        stock.AddPrice("2024-06-15", "100");

        chart.UpdateData(Some(&stock), &config);
        assert!(chart.desired_price.valid);
        assert!((chart.desired_price.value - 120.0).abs() < 1e-10);
        assert!(chart.desired_price_text.contains("Desired Price"));
    }

    #[test]
    fn chart_transformation_valid() {
        let mut chart = emStocksItemChart::for_test();
        chart.selected_date = "2024-06-15".to_string();
        let config = emStocksConfig {
            chart_period: ChartPeriod::Week1,
            ..Default::default()
        };
        let mut stock = StockRec::default();
        stock.AddPrice("2024-06-10", "100");
        stock.AddPrice("2024-06-15", "110");

        chart.UpdateData(Some(&stock), &config);
        // Y factor should be negative (price increases upward)
        assert!(chart.y_factor < 0.0);
        assert!(chart.upper_price > chart.lower_price);
    }

    #[test]
    fn chart_invalidate_resets_flag() {
        let mut chart = emStocksItemChart::for_test();
        let config = emStocksConfig::default();
        chart.UpdateData(None, &config);
        assert!(chart.data_up_to_date);
        chart.InvalidateData();
        assert!(!chart.data_up_to_date);
    }

    // ------------------------------------------------------------------
    // Coordinate transform and helper tests
    // ------------------------------------------------------------------

    #[test]
    fn view_context_methods() {
        let mut chart = emStocksItemChart::for_test();
        chart.set_view_context(800.0, 400.0, true);
        assert!((chart.ViewToPanelDeltaX(14.0) - 14.0 / 800.0).abs() < 1e-12);
        assert!((chart.ViewToPanelDeltaY(14.0) - 14.0 / 400.0).abs() < 1e-12);
        assert!((chart.PanelToViewDeltaY(0.01) - 0.01 * 400.0).abs() < 1e-12);
    }

    #[test]
    fn calculate_y_scale_level_range_basic() {
        let mut chart = emStocksItemChart::for_test();
        // Set up a chart with price range 50..150
        chart.lower_price = 50.0;
        chart.upper_price = 150.0;
        chart.y_factor = -0.01; // negative = price increases upward
        chart.set_view_context(800.0, 400.0, true);

        let (min_level, min_dist, max_level) = chart.CalculateYScaleLevelRange();

        // With range 100, f = 100*0.4 = 40. maxDist starts at 1, grows to 10, then stops
        // (100 > 40). So maxDist=10, maxLevel=2.
        assert!(
            max_level >= min_level,
            "max_level({}) >= min_level({})",
            max_level,
            min_level
        );
        assert!(min_dist > 0.0, "min_dist should be positive");
    }

    #[test]
    fn calculate_y_scale_level_range_small_range() {
        let mut chart = emStocksItemChart::for_test();
        chart.lower_price = 99.0;
        chart.upper_price = 101.0;
        chart.y_factor = -0.5;
        chart.set_view_context(800.0, 4000.0, true); // high zoom

        let (min_level, min_dist, max_level) = chart.CalculateYScaleLevelRange();
        assert!(max_level >= min_level);
        // With small price range, min_dist should be small
        assert!(
            min_dist <= 1.0,
            "min_dist={} should be <= 1.0 for small range",
            min_dist
        );
    }

    #[test]
    fn paint_content_no_crash_empty() {
        let mut chart = emStocksItemChart::for_test();
        chart.set_view_context(800.0, 400.0, true);
        let mut img = emcore::emImage::emImage::new(100, 100, 4);
        let mut painter = emPainter::new(&mut img);
        // Should not panic with default (empty) chart
        chart.PaintContent(&mut painter);
    }

    #[test]
    fn paint_content_no_crash_with_data() {
        let mut chart = emStocksItemChart::for_test();
        chart.selected_date = "2024-06-15".to_string();
        let config = emStocksConfig {
            chart_period: ChartPeriod::Week1,
            ..Default::default()
        };
        let mut stock = StockRec::default();
        stock.owning_shares = true;
        stock.trade_price = "50.00".to_string();
        stock.trade_date = "2024-06-12".to_string();
        stock.desired_price = "60.00".to_string();
        stock.AddPrice("2024-06-10", "48");
        stock.AddPrice("2024-06-11", "49");
        stock.AddPrice("2024-06-12", "50");
        stock.AddPrice("2024-06-13", "52");
        stock.AddPrice("2024-06-14", "54");
        stock.AddPrice("2024-06-15", "55");

        chart.UpdateData(Some(&stock), &config);
        chart.set_view_context(800.0, 400.0, true);

        let mut img = emcore::emImage::emImage::new(200, 100, 4);
        let mut painter = emPainter::new(&mut img);
        // Should not panic
        chart.PaintContent(&mut painter);
    }

    #[test]
    fn paint_desired_price_no_crash() {
        let mut chart = emStocksItemChart::for_test();
        chart.selected_date = "2024-06-15".to_string();
        let config = emStocksConfig {
            chart_period: ChartPeriod::Week1,
            ..Default::default()
        };
        let mut stock = StockRec::default();
        stock.desired_price = "100.00".to_string();
        stock.AddPrice("2024-06-15", "95");
        chart.UpdateData(Some(&stock), &config);
        chart.set_view_context(800.0, 400.0, true);

        let mut img = emcore::emImage::emImage::new(200, 100, 4);
        let mut painter = emPainter::new(&mut img);
        chart.PaintDesiredPrice(&mut painter);
    }

    #[test]
    fn paint_graph_no_crash() {
        let mut chart = emStocksItemChart::for_test();
        chart.selected_date = "2024-06-15".to_string();
        let config = emStocksConfig {
            chart_period: ChartPeriod::Week1,
            ..Default::default()
        };
        let mut stock = StockRec::default();
        stock.AddPrice("2024-06-10", "100");
        stock.AddPrice("2024-06-11", "102");
        stock.AddPrice("2024-06-12", "101");
        stock.AddPrice("2024-06-13", "105");
        stock.AddPrice("2024-06-14", "103");
        stock.AddPrice("2024-06-15", "107");
        chart.UpdateData(Some(&stock), &config);
        chart.set_view_context(800.0, 400.0, true);

        let mut img = emcore::emImage::emImage::new(200, 100, 4);
        let mut painter = emPainter::new(&mut img);
        chart.PaintGraph(&mut painter);
    }

    #[test]
    fn paint_price_bar_profit_and_loss() {
        // Test both profit (owning + price up) and loss (owning + price down)
        for (price_str, expected_profit) in &[("55", true), ("45", false)] {
            let mut chart = emStocksItemChart::for_test();
            chart.selected_date = "2024-06-15".to_string();
            let config = emStocksConfig {
                chart_period: ChartPeriod::Week1,
                ..Default::default()
            };
            let mut stock = StockRec::default();
            stock.owning_shares = true;
            stock.trade_price = "50.00".to_string();
            stock.trade_date = "2024-06-12".to_string();
            stock.AddPrice("2024-06-15", price_str);
            chart.UpdateData(Some(&stock), &config);
            chart.set_view_context(800.0, 400.0, true);

            assert!(chart.price_on_selected_date.valid, "price should be valid");
            let y1 = chart.y_offset + chart.y_factor * chart.trade_price.value;
            let y2 = chart.y_offset + chart.y_factor * chart.price_on_selected_date.value;
            // In the coordinate system y1 > y2 means profit (price went up)
            assert_eq!(
                y1 > y2,
                *expected_profit,
                "profit check for price={}",
                price_str
            );

            let mut img = emcore::emImage::emImage::new(200, 100, 4);
            let mut painter = emPainter::new(&mut img);
            chart.PaintPriceBar(&mut painter);
        }
    }
}
