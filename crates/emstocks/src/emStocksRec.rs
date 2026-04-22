// Port of C++ emStocksRec.h / emStocksRec.cpp

use std::fmt;
use std::str::FromStr;

use emcore::emCrossPtr::emCrossPtrList;
use emcore::emRecParser::{RecStruct, RecValue};
use emcore::emRecRecord::{RecError, Record};

// ─── Interest ────────────────────────────────────────────────────────────────

/// Port of C++ emStocksRec::InterestType + InterestRec.
/// Rust enum replaces C++ int enum + emEnumRec subclass — Rust enums are the idiomatic equivalent of C++ int enums with associated string tables.
/// Deprecated identifier handling via explicit methods rather than virtual TryStartReading.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Interest {
    High = 0,
    #[default]
    Medium = 1,
    Low = 2,
}

impl fmt::Display for Interest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::High => write!(f, "HIGH"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::Low => write!(f, "LOW"),
        }
    }
}

impl FromStr for Interest {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "HIGH" => Ok(Self::High),
            "MEDIUM" => Ok(Self::Medium),
            "LOW" => Ok(Self::Low),
            _ => Err(format!("unknown interest: {s}")),
        }
    }
}

impl Interest {
    /// C++ buggy deprecated mapping (bugInDeprecatedIdentifiers=true):
    /// "LOW_INTEREST" → High (bug), "HIGH_INTEREST" → Low (bug),
    /// "MEDIUM_INTEREST" → Medium. Case-insensitive.
    pub fn from_deprecated_bugged(s: &str) -> Self {
        let upper = s.to_ascii_uppercase();
        match upper.as_str() {
            "LOW_INTEREST" => Self::High, // C++ bug: swapped
            "HIGH_INTEREST" => Self::Low, // C++ bug: swapped
            "MEDIUM_INTEREST" => Self::Medium,
            _ => Self::Medium,
        }
    }

    /// Normal deprecated mapping (no bug):
    /// "LOW_INTEREST" → Low, "HIGH_INTEREST" → High. Case-insensitive.
    pub fn from_deprecated_normal(s: &str) -> Self {
        let upper = s.to_ascii_uppercase();
        match upper.as_str() {
            "LOW_INTEREST" => Self::Low,
            "HIGH_INTEREST" => Self::High,
            "MEDIUM_INTEREST" => Self::Medium,
            _ => Self::Medium,
        }
    }

    /// Try to parse from a rec identifier (lowercase from RecStruct).
    /// Tries canonical names first, then deprecated-with-bug (matching C++ constructor).
    fn from_rec_ident(s: &str) -> Self {
        let upper = s.to_ascii_uppercase();
        if let Ok(interest) = Interest::from_str(&upper) {
            return interest;
        }
        Interest::from_deprecated_bugged(&upper)
    }
}

// ─── Date arithmetic ─────────────────────────────────────────────────────────
// Port of C++ emStocksRec static methods as standalone pub functions.

/// Parse "YYYY-MM-DD" format. Returns (year, month, day) if valid.
/// Handles negative years (leading '-'). C++ returns bool for validity.
pub fn ParseDate(date: &str) -> Option<(i32, i32, i32)> {
    let bytes = date.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut s: i32 = 1;
    let (mut y, mut m, mut d): (i32, i32, i32) = (0, 0, 0);

    // Skip non-digits, check for '-'
    while i < len && (bytes[i] < b'0' || bytes[i] > b'9') {
        if bytes[i] == b'-' {
            s = -1;
        }
        i += 1;
    }
    // Read year digits
    while i < len && bytes[i] >= b'0' && bytes[i] <= b'9' {
        y = y * 10 + (bytes[i] - b'0') as i32;
        i += 1;
    }
    // Skip non-digits
    while i < len && (bytes[i] < b'0' || bytes[i] > b'9') {
        i += 1;
    }
    // Read month digits
    while i < len && bytes[i] >= b'0' && bytes[i] <= b'9' {
        m = m * 10 + (bytes[i] - b'0') as i32;
        i += 1;
    }
    // Skip non-digits
    while i < len && (bytes[i] < b'0' || bytes[i] > b'9') {
        i += 1;
    }
    // Read day digits
    while i < len && bytes[i] >= b'0' && bytes[i] <= b'9' {
        d = d * 10 + (bytes[i] - b'0') as i32;
        i += 1;
    }

    if m >= 1 && d >= 1 {
        Some((s * y, m, d))
    } else {
        None
    }
}

/// Compare two date strings. Returns negative if date1 < date2, positive if date1 > date2, 0 if equal.
/// Uses C++ formula: ((y1-y2)*16+m1-m2)*32+d1-d2
pub fn CompareDates(date1: &str, date2: &str) -> i32 {
    let (y1, m1, d1) = ParseDate(date1).unwrap_or((0, 0, 0));
    let (y2, m2, d2) = ParseDate(date2).unwrap_or((0, 0, 0));
    ((y1 - y2) * 16 + m1 - m2) * 32 + d1 - d2
}

/// Days in a month, accounting for leap years.
pub fn GetDaysOfMonth(year: i32, month: i32) -> i32 {
    match month {
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Add days to a date (mutating y/m/d). Port the C++ boundary loops exactly.
pub fn AddDaysToDateParts(days: i32, y: &mut i32, m: &mut i32, d: &mut i32) {
    *d += days;
    while *d < -213 {
        *d += 365 - 28 + GetDaysOfMonth(if *m > 2 { *y } else { *y - 1 }, 2);
        *y -= 1;
    }
    while *d > 243 {
        *y += 1;
        *d -= 365 - 28 + GetDaysOfMonth(if *m > 2 { *y } else { *y - 1 }, 2);
    }
    while *d < 1 {
        *m -= 1;
        if *m < 1 {
            *y -= 1;
            *m = 12;
        }
        *d += GetDaysOfMonth(*y, *m);
    }
    while *d > 28 {
        let n = GetDaysOfMonth(*y, *m);
        if *d <= n {
            break;
        }
        *d -= n;
        *m += 1;
        if *m > 12 {
            *y += 1;
            *m = 1;
        }
    }
}

/// Add days to a date string, returning new date string "YYYY-MM-DD".
pub fn AddDaysToDate(days: i32, date: &str) -> String {
    let (mut y, mut m, mut d) = ParseDate(date).unwrap_or((0, 0, 0));
    AddDaysToDateParts(days, &mut y, &mut m, &mut d);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Get difference in days between two dates (6-arg version).
pub fn GetDateDifferenceParts(
    mut y1: i32,
    mut m1: i32,
    mut d1: i32,
    mut y2: i32,
    mut m2: i32,
    d2: i32,
) -> i32 {
    let mut days = d2 - d1;
    if y1 != y2 {
        days += (y2 - y1) * 365 + (m2 - m1) * 30;
        AddDaysToDateParts(days, &mut y1, &mut m1, &mut d1);
        days += d2 - d1;
    }
    while y1 < y2 || (y1 == y2 && m1 < m2) {
        days += GetDaysOfMonth(y1, m1);
        m1 += 1;
        if m1 > 12 {
            y1 += 1;
            m1 = 1;
        }
    }
    while y1 > y2 || (y1 == y2 && m1 > m2) {
        days -= GetDaysOfMonth(y2, m2);
        m2 += 1;
        if m2 > 12 {
            y2 += 1;
            m2 = 1;
        }
    }
    days
}

/// Get difference in days between two date strings.
/// Returns (i32, bool) tuple instead of C++ bool* out-param — Rust has no out-parameters; tuples are the idiomatic equivalent.
pub fn GetDateDifference(from_date: &str, to_date: &str) -> (i32, bool) {
    let from_parsed = ParseDate(from_date);
    let to_parsed = ParseDate(to_date);
    let (y1, m1, d1) = from_parsed.unwrap_or((0, 0, 0));
    let (y2, m2, d2) = to_parsed.unwrap_or((0, 0, 0));
    let valid = from_parsed.is_some() && to_parsed.is_some();
    (GetDateDifferenceParts(y1, m1, d1, y2, m2, d2), valid)
}

/// Get current date as "YYYY-MM-DD".
pub fn GetCurrentDate() -> String {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tmbuf: libc::tm = std::mem::zeroed();
        let p = libc::localtime_r(&t, &mut tmbuf);
        if p.is_null() {
            return "0000-00-00".to_string();
        }
        format!(
            "{:04}-{:02}-{:02}",
            (*p).tm_year + 1900,
            (*p).tm_mon + 1,
            (*p).tm_mday
        )
    }
}

// ─── Price formatting ────────────────────────────────────────────────────────

/// Format share price with adaptive decimal places based on magnitude.
pub fn SharePriceToString(price: f64) -> String {
    let mut d = 0;
    let mut m = 1000.0_f64;
    loop {
        if price.abs() >= m {
            break;
        }
        if d >= 8 {
            if price == 0.0 {
                d = 0;
            }
            break;
        }
        d += 1;
        m /= 10.0;
    }
    format!("{:.prec$}", price, prec = d)
}

/// Format payment price with 2 decimal places.
pub fn PaymentPriceToString(price: f64) -> String {
    format!("{:.2}", price)
}

// ─── StockRec ────────────────────────────────────────────────────────────────

/// Port of C++ emStocksRec::StockRec.
/// DIVERGED: (language-forced) Rust struct fields use snake_case — required by Rust naming conventions (clippy::non_snake_case). Method names preserve C++ names per File and Name Correspondence.
#[derive(Default)]
pub struct StockRec {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub wkn: String,
    pub isin: String,
    pub country: String,
    pub sector: String,
    pub collection: String,
    pub comment: String,
    pub owning_shares: bool,
    pub own_shares: String,
    pub trade_price: String,
    pub trade_date: String,
    pub prices: String,
    pub last_price_date: String,
    pub desired_price: String,
    pub expected_dividend: String,
    pub inquiry_date: String,
    pub interest: Interest,
    pub web_pages: Vec<String>,
    cross_ptr_list: emCrossPtrList,
}

impl Clone for StockRec {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            symbol: self.symbol.clone(),
            wkn: self.wkn.clone(),
            isin: self.isin.clone(),
            country: self.country.clone(),
            sector: self.sector.clone(),
            collection: self.collection.clone(),
            comment: self.comment.clone(),
            owning_shares: self.owning_shares,
            own_shares: self.own_shares.clone(),
            trade_price: self.trade_price.clone(),
            trade_date: self.trade_date.clone(),
            prices: self.prices.clone(),
            last_price_date: self.last_price_date.clone(),
            desired_price: self.desired_price.clone(),
            expected_dividend: self.expected_dividend.clone(),
            inquiry_date: self.inquiry_date.clone(),
            interest: self.interest,
            web_pages: self.web_pages.clone(),
            cross_ptr_list: emCrossPtrList::new(),
        }
    }
}

impl fmt::Debug for StockRec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StockRec")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("symbol", &self.symbol)
            .field("wkn", &self.wkn)
            .field("isin", &self.isin)
            .field("country", &self.country)
            .field("sector", &self.sector)
            .field("collection", &self.collection)
            .field("comment", &self.comment)
            .field("owning_shares", &self.owning_shares)
            .field("own_shares", &self.own_shares)
            .field("trade_price", &self.trade_price)
            .field("trade_date", &self.trade_date)
            .field("prices", &self.prices)
            .field("last_price_date", &self.last_price_date)
            .field("desired_price", &self.desired_price)
            .field("expected_dividend", &self.expected_dividend)
            .field("inquiry_date", &self.inquiry_date)
            .field("interest", &self.interest)
            .field("web_pages", &self.web_pages)
            .finish()
    }
}

impl StockRec {
    pub const MAX_NUM_PRICES: usize = 366 * 20;

    /// Expose cross-pointer list for linking. Corresponds to C++ LinkCrossPtr.
    pub fn LinkCrossPtr(&mut self) -> &mut emCrossPtrList {
        &mut self.cross_ptr_list
    }

    /// Port of C++ GetPricePtrOfDate. Returns the price substring for a given date.
    /// Returns "" if date is out of range.
    pub fn GetPricePtrOfDate(&self, date: &str) -> &str {
        let (mut d, dates_valid) = GetDateDifference(date, &self.last_price_date);
        if !dates_valid || d < 0 {
            return "";
        }
        // Walk backwards through pipe-separated segments
        let prices = self.prices.as_bytes();
        let mut end = prices.len();
        loop {
            if end == 0 {
                return "";
            }
            // Find start of this segment
            let mut start = end;
            while start > 0 && prices[start - 1] != b'|' {
                start -= 1;
            }
            d -= 1;
            if d < 0 {
                return &self.prices[start..end];
            }
            // Move past the '|' separator
            if start > 0 {
                end = start - 1;
            } else {
                end = 0;
            }
        }
    }

    /// Port of C++ GetPriceOfDate.
    pub fn GetPriceOfDate(&self, date: &str) -> String {
        let p = self.GetPricePtrOfDate(date);
        // In C++, GetPriceOfDate extracts up to '|', but GetPricePtrOfDate
        // already returns a segment without '|', so just return it.
        p.to_string()
    }

    /// Port of C++ GetPricesDateBefore.
    pub fn GetPricesDateBefore(&self, date: &str) -> String {
        let (d, _) = GetDateDifference(date, &self.last_price_date);
        let prices = self.prices.as_bytes();
        let mut end = prices.len();
        let mut n: i32 = 0;
        while end > 0 {
            let mut start = end;
            while start > 0 && prices[start - 1] != b'|' {
                start -= 1;
            }
            if n > d {
                // Check segment is non-empty
                let seg = &self.prices[start..end];
                if !seg.is_empty() && seg.as_bytes()[0] != b'|' {
                    return AddDaysToDate(-n, &self.last_price_date);
                }
            }
            n += 1;
            if start > 0 {
                end = start - 1;
            } else {
                end = 0;
            }
        }
        String::new()
    }

    /// Port of C++ GetPricesDateAfter.
    pub fn GetPricesDateAfter(&self, date: &str) -> String {
        let (d, _) = GetDateDifference(date, &self.last_price_date);
        if d <= 0 {
            return String::new();
        }
        let prices = self.prices.as_bytes();
        let mut end = prices.len();
        let mut n: i32 = 0;
        let mut m: i32 = -1;
        while end > 0 {
            let mut start = end;
            while start > 0 && prices[start - 1] != b'|' {
                start -= 1;
            }
            let seg = &self.prices[start..end];
            if !seg.is_empty() && seg.as_bytes()[0] != b'|' {
                m = n;
            }
            if n + 1 >= d {
                break;
            }
            n += 1;
            if start > 0 {
                end = start - 1;
            } else {
                end = 0;
            }
        }
        if m >= 0 {
            AddDaysToDate(-m, &self.last_price_date)
        } else {
            String::new()
        }
    }

    /// Port of C++ AddPrice. Complex method with MAX_NUM_PRICES trimming.
    pub fn AddPrice(&mut self, date: &str, price: &str) {
        let mut prices = self.prices.clone();

        // Count segments
        let mut n: i32 = 0;
        if !prices.is_empty() {
            n = 1;
            for b in prices.bytes() {
                if b == b'|' {
                    n += 1;
                }
            }
        }

        if n <= 0 {
            self.prices = price.to_string();
            self.last_price_date = date.to_string();
            return;
        }

        let mut i: i32 = n - 1 + GetDateDifference(&self.last_price_date, date).0;

        if i >= n {
            // Trim old prices from front if exceeding MAX_NUM_PRICES
            let bytes = prices.as_bytes();
            let mut pos = 0;
            while pos < bytes.len()
                && ((i + 1) as usize > Self::MAX_NUM_PRICES || bytes[pos] == b'|')
            {
                loop {
                    pos += 1;
                    if pos >= bytes.len() || bytes[pos - 1] == b'|' {
                        break;
                    }
                }
                n -= 1;
                i -= 1;
            }
            if n <= 0 {
                self.prices = price.to_string();
                self.last_price_date = date.to_string();
                return;
            }
            if pos > 0 {
                prices = prices[pos..].to_string();
            }
        }

        if i < 0 {
            // Trim recent prices from back if exceeding MAX_NUM_PRICES
            let bytes = prices.as_bytes();
            let mut q_pos = bytes.len();
            while q_pos > 0
                && ((-i + n) as usize > Self::MAX_NUM_PRICES || bytes[q_pos - 1] == b'|')
            {
                loop {
                    q_pos -= 1;
                    if q_pos == 0 || bytes[q_pos] == b'|' {
                        break;
                    }
                }
                n -= 1;
                self.last_price_date = AddDaysToDate(-1, &self.last_price_date);
            }
            if n <= 0 {
                self.prices = price.to_string();
                self.last_price_date = date.to_string();
                return;
            }
            if q_pos < bytes.len() {
                prices = prices[..q_pos].to_string();
            }
        }

        if i >= n {
            // Extend with pipe separators
            let extend = (i + 1 - n) as usize;
            for _ in 0..extend {
                prices.push('|');
            }
            n = i + 1;
            self.last_price_date = date.to_string();
        }

        if i < 0 {
            // Prepend pipe separators
            let prepend = (-i) as usize;
            let mut prefix = String::with_capacity(prepend + prices.len());
            for _ in 0..prepend {
                prefix.push('|');
            }
            prefix.push_str(&prices);
            prices = prefix;
            n += -i;
            i = 0;
        }

        // Replace segment at index i (counting from end, j = n-1 is last segment)
        // Find the segment at position i (0 = first/oldest, n-1 = last/newest)
        let bytes = prices.as_bytes();
        let total_len = bytes.len();
        let mut e_pos = total_len;
        let mut q_pos;
        let mut j = n - 1;
        loop {
            q_pos = e_pos;
            while q_pos > 0 && bytes[q_pos - 1] != b'|' {
                q_pos -= 1;
            }
            if j <= i {
                break;
            }
            e_pos = if q_pos > 0 { q_pos - 1 } else { 0 };
            j -= 1;
        }

        // Replace prices[q_pos..e_pos] with price
        let mut result = String::with_capacity(q_pos + price.len() + (total_len - e_pos));
        result.push_str(&prices[..q_pos]);
        result.push_str(price);
        result.push_str(&prices[e_pos..]);
        self.prices = result;
    }

    /// Port of C++ IsMatchingSearchText. Case-insensitive substring search.
    pub fn IsMatchingSearchText(&self, search_text: &str) -> bool {
        let needle = search_text.to_ascii_lowercase();
        let fields = [
            &self.name,
            &self.symbol,
            &self.wkn,
            &self.isin,
            &self.country,
            &self.sector,
            &self.collection,
            &self.comment,
        ];
        for field in &fields {
            if field.to_ascii_lowercase().contains(&needle) {
                return true;
            }
        }
        false
    }

    /// Port of C++ GetTradeValue.
    /// Idiom adaptation: returns Option<f64> instead of C++ bool + *pResult out-pointer.
    pub fn GetTradeValue(&self) -> Option<f64> {
        if !self.owning_shares || self.trade_price.is_empty() || self.own_shares.is_empty() {
            return None;
        }
        let tp: f64 = self.trade_price.parse().unwrap_or(0.0);
        let os: f64 = self.own_shares.parse().unwrap_or(0.0);
        Some(tp * os)
    }

    /// Port of C++ GetValueOfDate.
    pub fn GetValueOfDate(&self, date: &str) -> Option<f64> {
        if !self.owning_shares || self.own_shares.is_empty() {
            return None;
        }
        let price_str = self.GetPricePtrOfDate(date);
        let first = price_str.as_bytes().first().copied().unwrap_or(0);
        if !first.is_ascii_digit() {
            return None;
        }
        let p: f64 = price_str.parse().unwrap_or(0.0);
        let os: f64 = self.own_shares.parse().unwrap_or(0.0);
        Some(p * os)
    }

    /// Port of C++ GetDifferenceValueOfDate.
    pub fn GetDifferenceValueOfDate(&self, date: &str) -> Option<f64> {
        let v1 = self.GetTradeValue()?;
        let v2 = self.GetValueOfDate(date)?;
        Some(v2 - v1)
    }

    /// Port of C++ GetAchievementOfDate.
    pub fn GetAchievementOfDate(&self, date: &str, relative: bool) -> Option<f64> {
        if self.desired_price.is_empty() {
            return None;
        }
        let mut d: f64 = self.desired_price.parse().unwrap_or(0.0);
        if d < 1e-10 {
            return None;
        }

        let price_str = self.GetPricePtrOfDate(date);
        let first = price_str.as_bytes().first().copied().unwrap_or(0);
        if !first.is_ascii_digit() {
            return None;
        }
        let p: f64 = price_str.parse().unwrap_or(0.0);
        if p < 1e-10 {
            return None;
        }

        let result = if relative {
            if self.trade_price.is_empty() {
                return None;
            }
            let t: f64 = self.trade_price.parse().unwrap_or(0.0);
            if t < 1e-10 {
                return None;
            }
            if (d - t).abs() < 1e-10 {
                d = t + if self.owning_shares { 1e-10 } else { -1e-10 };
            }
            (p - t) / (d - t)
        } else if self.owning_shares {
            p / d
        } else {
            d / p
        };

        Some(result * 100.0)
    }

    /// Port of C++ GetRiseUntilDate.
    pub fn GetRiseUntilDate(&self, date: &str, days: i32) -> Option<f64> {
        let q_str = self.GetPricePtrOfDate(date);
        let first_byte = q_str.as_bytes().first().copied().unwrap_or(0);
        if !first_byte.is_ascii_digit() {
            return None;
        }
        let c_val: f64 = q_str.parse().unwrap_or(0.0);
        if c_val < 1e-10 {
            return None;
        }

        // Collect all segments in order (oldest first, same as self.prices layout)
        let segments: Vec<&str> = self.prices.split('|').collect();

        // Find which segment index corresponds to `date`.
        // GetPricePtrOfDate returns segment at index (total_segments - 1 - diff)
        // where diff = GetDateDifference(date, last_price_date).
        let (diff, _) = GetDateDifference(date, &self.last_price_date);
        let total = segments.len() as i32;
        let cur_idx = total - 1 - diff;

        let d1 = days - days / 6;
        let d2 = days + days / 6;
        let mut m = 0.0_f64;
        let mut n = 0;
        let mut last_valid_val: Option<f64> = None;

        // Walk backwards from cur_idx-1 (d starts at 1)
        for d in 1..=d2 {
            let idx = cur_idx - d;
            if idx < 0 {
                break;
            }
            let seg = segments[idx as usize];
            let seg_first = seg.as_bytes().first().copied().unwrap_or(0);
            if !seg_first.is_ascii_digit() {
                continue;
            }
            last_valid_val = Some(seg.parse::<f64>().unwrap_or(0.0));
            if d < d1 {
                continue;
            }
            m += seg.parse::<f64>().unwrap_or(0.0);
            n += 1;
        }

        if n == 0 {
            m = last_valid_val.unwrap_or(c_val);
        } else {
            m /= n as f64;
        }

        if m < 1e-10 {
            return None;
        }

        let c_result = if self.owning_shares {
            c_val / m
        } else {
            m / c_val
        };

        Some(c_result * 100.0)
    }
}

impl PartialEq for StockRec {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.symbol == other.symbol
            && self.wkn == other.wkn
            && self.isin == other.isin
            && self.country == other.country
            && self.sector == other.sector
            && self.collection == other.collection
            && self.comment == other.comment
            && self.owning_shares == other.owning_shares
            && self.own_shares == other.own_shares
            && self.trade_price == other.trade_price
            && self.trade_date == other.trade_date
            && self.prices == other.prices
            && self.last_price_date == other.last_price_date
            && self.desired_price == other.desired_price
            && self.expected_dividend == other.expected_dividend
            && self.inquiry_date == other.inquiry_date
            && self.interest == other.interest
            && self.web_pages == other.web_pages
    }
}

impl Record for StockRec {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let get_str = |name: &str| -> String { rec.get_str(name).unwrap_or("").to_string() };

        let interest = if let Some(ident) = rec.get_ident("Interest") {
            Interest::from_rec_ident(ident)
        } else {
            Interest::default()
        };

        let web_pages = if let Some(arr) = rec.get_array("WebPages") {
            arr.iter()
                .filter_map(|v| {
                    if let RecValue::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(Self {
            id: get_str("Id"),
            name: get_str("Name"),
            symbol: get_str("Symbol"),
            wkn: get_str("WKN"),
            isin: get_str("ISIN"),
            country: get_str("Country"),
            sector: get_str("Sector"),
            collection: get_str("Collection"),
            comment: get_str("Comment"),
            owning_shares: rec.get_bool("OwningShares").unwrap_or(false),
            own_shares: get_str("OwnShares"),
            trade_price: get_str("TradePrice"),
            trade_date: get_str("TradeDate"),
            prices: get_str("Prices"),
            last_price_date: get_str("LastPriceDate"),
            desired_price: get_str("DesiredPrice"),
            expected_dividend: get_str("ExpectedDividend"),
            inquiry_date: get_str("InquiryDate"),
            interest,
            web_pages,
            cross_ptr_list: emCrossPtrList::new(),
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut rec = RecStruct::new();
        rec.set_str("Id", &self.id);
        rec.set_str("Name", &self.name);
        rec.set_str("Symbol", &self.symbol);
        rec.set_str("WKN", &self.wkn);
        rec.set_str("ISIN", &self.isin);
        rec.set_str("Country", &self.country);
        rec.set_str("Sector", &self.sector);
        rec.set_str("Collection", &self.collection);
        rec.set_str("Comment", &self.comment);
        rec.set_bool("OwningShares", self.owning_shares);
        rec.set_str("OwnShares", &self.own_shares);
        rec.set_str("TradePrice", &self.trade_price);
        rec.set_str("TradeDate", &self.trade_date);
        rec.set_str("Prices", &self.prices);
        rec.set_str("LastPriceDate", &self.last_price_date);
        rec.set_str("DesiredPrice", &self.desired_price);
        rec.set_str("ExpectedDividend", &self.expected_dividend);
        rec.set_str("InquiryDate", &self.inquiry_date);
        rec.set_ident("Interest", &self.interest.to_string());
        rec.SetValue(
            "WebPages",
            RecValue::Array(
                self.web_pages
                    .iter()
                    .map(|s| RecValue::Str(s.clone()))
                    .collect(),
            ),
        );
        rec
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

// ─── emStocksRec ─────────────────────────────────────────────────────────────

/// Port of C++ emStocksRec.
#[derive(Default)]
pub struct emStocksRec {
    pub stocks: Vec<StockRec>,
}

impl emStocksRec {
    /// Port of C++ GetFormatName.
    pub fn GetFormatName(&self) -> &str {
        "emStocks"
    }

    /// Port of C++ InventStockId.
    /// Finds max ID + 1. If overflow, finds first unused ID.
    pub fn InventStockId(&self) -> String {
        let mut id: i32 = 0;
        for stock in &self.stocks {
            let parsed: i32 = stock.id.parse().unwrap_or(0);
            id = id.max(parsed);
        }
        if id < i32::MAX {
            id += 1;
        } else {
            // Find first unused ID (extremely unlikely path)
            let mut candidate: i32 = 0;
            loop {
                let s = candidate.to_string();
                if !self.stocks.iter().any(|st| st.id == s) {
                    return s;
                }
                candidate += 1;
            }
        }
        format!("{}", id)
    }

    /// Port of C++ GetStockIndexById.
    /// Idiom adaptation: returns Option<usize> instead of C++ -1 sentinel.
    pub fn GetStockIndexById(&self, id: &str) -> Option<usize> {
        (0..self.stocks.len())
            .rev()
            .find(|&i| self.stocks[i].id == id)
    }

    /// Port of C++ GetStockIndexByStock.
    /// Uses std::ptr::eq for pointer comparison.
    /// Idiom adaptation: returns Option<usize> instead of C++ -1 sentinel.
    pub fn GetStockIndexByStock(&self, stock_rec: &StockRec) -> Option<usize> {
        (0..self.stocks.len())
            .rev()
            .find(|&i| std::ptr::eq(&self.stocks[i], stock_rec))
    }

    /// Port of C++ GetLatestPricesDate. Scans all stocks.
    pub fn GetLatestPricesDate(&self) -> String {
        let mut result = String::new();
        for stock in &self.stocks {
            if result.is_empty() || CompareDates(&stock.last_price_date, &result) > 0 {
                result = stock.last_price_date.clone();
            }
        }
        result
    }

    /// Port of C++ GetPricesDateBefore.
    pub fn GetPricesDateBefore(&self, date: &str) -> String {
        let mut result = String::new();
        for stock in &self.stocks {
            let d = stock.GetPricesDateBefore(date);
            if !d.is_empty() && (result.is_empty() || CompareDates(&d, &result) > 0) {
                result = d;
            }
        }
        result
    }

    /// Port of C++ GetPricesDateAfter.
    pub fn GetPricesDateAfter(&self, date: &str) -> String {
        let mut result = String::new();
        for stock in &self.stocks {
            let d = stock.GetPricesDateAfter(date);
            if !d.is_empty() && (result.is_empty() || CompareDates(&d, &result) < 0) {
                result = d;
            }
        }
        result
    }
}

impl Record for emStocksRec {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let stocks = if let Some(arr) = rec.get_array("Stocks") {
            arr.iter()
                .filter_map(|v| {
                    if let RecValue::Struct(s) = v {
                        StockRec::from_rec(s).ok()
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok(Self { stocks })
    }

    fn to_rec(&self) -> RecStruct {
        let mut rec = RecStruct::new();
        rec.SetValue(
            "Stocks",
            RecValue::Array(
                self.stocks
                    .iter()
                    .map(|s| RecValue::Struct(s.to_rec()))
                    .collect(),
            ),
        );
        rec
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        self.stocks.is_empty()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interest_from_str_canonical() {
        assert_eq!(Interest::from_str("HIGH"), Ok(Interest::High));
        assert_eq!(Interest::from_str("MEDIUM"), Ok(Interest::Medium));
        assert_eq!(Interest::from_str("LOW"), Ok(Interest::Low));
    }

    #[test]
    fn interest_from_str_deprecated_with_bug() {
        assert_eq!(
            Interest::from_deprecated_bugged("LOW_INTEREST"),
            Interest::High
        );
        assert_eq!(
            Interest::from_deprecated_bugged("HIGH_INTEREST"),
            Interest::Low
        );
        assert_eq!(
            Interest::from_deprecated_bugged("MEDIUM_INTEREST"),
            Interest::Medium
        );
    }

    #[test]
    fn interest_from_str_deprecated_no_bug() {
        assert_eq!(
            Interest::from_deprecated_normal("LOW_INTEREST"),
            Interest::Low
        );
        assert_eq!(
            Interest::from_deprecated_normal("HIGH_INTEREST"),
            Interest::High
        );
    }

    #[test]
    fn interest_display() {
        assert_eq!(Interest::High.to_string(), "HIGH");
        assert_eq!(Interest::Medium.to_string(), "MEDIUM");
        assert_eq!(Interest::Low.to_string(), "LOW");
    }

    #[test]
    fn stock_rec_default() {
        let rec = StockRec::default();
        assert_eq!(rec.id, "");
        assert_eq!(rec.interest, Interest::Medium);
        assert!(rec.web_pages.is_empty());
    }

    #[test]
    fn parse_date_valid() {
        assert_eq!(ParseDate("2024-03-15"), Some((2024, 3, 15)));
    }

    #[test]
    fn parse_date_negative_year() {
        assert_eq!(ParseDate("-500-01-01"), Some((-500, 1, 1)));
    }

    #[test]
    fn parse_date_invalid() {
        assert_eq!(ParseDate(""), None);
        assert_eq!(ParseDate("foo"), None);
    }

    #[test]
    fn compare_dates() {
        assert!(CompareDates("2024-03-15", "2024-03-16") < 0);
        assert!(CompareDates("2024-03-16", "2024-03-15") > 0);
        assert_eq!(CompareDates("2024-03-15", "2024-03-15"), 0);
    }

    #[test]
    fn days_of_month() {
        assert_eq!(GetDaysOfMonth(2024, 2), 29); // leap year
        assert_eq!(GetDaysOfMonth(2023, 2), 28);
        assert_eq!(GetDaysOfMonth(2024, 1), 31);
        assert_eq!(GetDaysOfMonth(2024, 4), 30);
    }

    #[test]
    fn add_days_to_date() {
        assert_eq!(AddDaysToDate(1, "2024-03-31"), "2024-04-01");
        assert_eq!(AddDaysToDate(-1, "2024-01-01"), "2023-12-31");
        assert_eq!(AddDaysToDate(366, "2024-01-01"), "2025-01-01"); // 2024 is leap year
    }

    #[test]
    fn get_date_difference() {
        assert_eq!(GetDateDifference("2024-01-01", "2024-01-02"), (1, true));
        assert_eq!(GetDateDifference("2024-01-02", "2024-01-01"), (-1, true));
        assert_eq!(GetDateDifference("2024-01-01", "2025-01-01").0, 366); // leap year
    }

    #[test]
    fn stock_add_price_and_retrieve() {
        let mut stock = StockRec::default();
        stock.AddPrice("2024-03-15", "100.50");
        assert_eq!(stock.last_price_date, "2024-03-15");
        assert_eq!(stock.GetPriceOfDate("2024-03-15"), "100.50");
    }

    #[test]
    fn stock_add_multiple_prices() {
        let mut stock = StockRec::default();
        stock.AddPrice("2024-03-14", "99.00");
        stock.AddPrice("2024-03-15", "100.50");
        assert_eq!(stock.GetPriceOfDate("2024-03-14"), "99.00");
        assert_eq!(stock.GetPriceOfDate("2024-03-15"), "100.50");
        assert_eq!(stock.last_price_date, "2024-03-15");
    }

    #[test]
    fn stock_is_matching_search_text() {
        let stock = StockRec {
            name: "Apple Inc.".to_string(),
            symbol: "AAPL".to_string(),
            ..StockRec::default()
        };
        assert!(stock.IsMatchingSearchText("apple"));
        assert!(stock.IsMatchingSearchText("AAPL"));
        assert!(!stock.IsMatchingSearchText("GOOG"));
    }

    #[test]
    fn stock_get_trade_value() {
        let stock = StockRec {
            owning_shares: true,
            trade_price: "150.00".to_string(),
            own_shares: "10".to_string(),
            ..StockRec::default()
        };
        assert_eq!(stock.GetTradeValue(), Some(1500.0));
    }

    #[test]
    fn stock_get_trade_value_not_owning() {
        let stock = StockRec::default();
        assert_eq!(stock.GetTradeValue(), None);
    }

    #[test]
    fn share_price_to_string_large() {
        // C++: d=0 (fabs(1234.5) >= 1000.0), format "%.0f" -> "1234"
        assert_eq!(SharePriceToString(1234.5), "1234");
    }

    #[test]
    fn share_price_to_string_small() {
        // C++: d=4 (fabs(0.12345678) >= 0.1), format "%.4f" -> "0.1235"
        assert_eq!(SharePriceToString(0.12345678), "0.1235");
    }

    #[test]
    fn share_price_to_string_zero() {
        assert_eq!(SharePriceToString(0.0), "0");
    }

    #[test]
    fn payment_price_to_string() {
        assert_eq!(PaymentPriceToString(1234.5), "1234.50");
    }

    #[test]
    fn stock_rec_record_round_trip() {
        let rec = StockRec {
            id: "42".to_string(),
            name: "Test Stock".to_string(),
            symbol: "TST".to_string(),
            interest: Interest::High,
            web_pages: vec!["https://example.com".to_string()],
            ..StockRec::default()
        };

        let serialized = rec.to_rec();
        let deserialized = StockRec::from_rec(&serialized).unwrap();

        assert_eq!(deserialized.id, "42");
        assert_eq!(deserialized.name, "Test Stock");
        assert_eq!(deserialized.symbol, "TST");
        assert_eq!(deserialized.interest, Interest::High);
        assert_eq!(deserialized.web_pages, vec!["https://example.com"]);
    }

    #[test]
    fn emstocks_rec_default() {
        let rec = emStocksRec::default();
        assert!(rec.stocks.is_empty());
    }

    #[test]
    fn emstocks_rec_record_round_trip() {
        let mut rec = emStocksRec::default();
        let stock = StockRec {
            id: "1".to_string(),
            name: "Test".to_string(),
            ..StockRec::default()
        };
        rec.stocks.push(stock);

        let serialized = rec.to_rec();
        let deserialized = emStocksRec::from_rec(&serialized).unwrap();
        assert_eq!(deserialized.stocks.len(), 1);
        assert_eq!(deserialized.stocks[0].name, "Test");
    }

    #[test]
    fn emstocks_rec_format_name() {
        let rec = emStocksRec::default();
        assert_eq!(rec.GetFormatName(), "emStocks");
    }

    #[test]
    fn emstocks_rec_invent_stock_id() {
        let mut rec = emStocksRec::default();
        assert_eq!(rec.InventStockId(), "1");

        let stock = StockRec {
            id: "5".to_string(),
            ..StockRec::default()
        };
        rec.stocks.push(stock);
        assert_eq!(rec.InventStockId(), "6");
    }

    #[test]
    fn emstocks_rec_get_stock_index_by_id() {
        let mut rec = emStocksRec::default();
        let stock = StockRec {
            id: "42".to_string(),
            ..StockRec::default()
        };
        rec.stocks.push(stock);
        assert_eq!(rec.GetStockIndexById("42"), Some(0));
        assert_eq!(rec.GetStockIndexById("99"), None);
    }

    #[test]
    fn emstocks_rec_get_latest_prices_date() {
        let mut rec = emStocksRec::default();

        let s1 = StockRec {
            last_price_date: "2024-03-14".to_string(),
            ..StockRec::default()
        };
        rec.stocks.push(s1);

        let s2 = StockRec {
            last_price_date: "2024-03-16".to_string(),
            ..StockRec::default()
        };
        rec.stocks.push(s2);

        assert_eq!(rec.GetLatestPricesDate(), "2024-03-16");
    }
}
