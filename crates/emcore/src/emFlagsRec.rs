//! emFlagsRec — concrete emRec<i32> with named bit identifiers.
//!
//! C++ reference: `include/emCore/emRec.h:643` (`class emFlagsRec : public emRec`)
//! and `src/emCore/emRec.cpp:755-917` for ctor/Set/GetIdentifierOf/GetBitOf/Init.
//!
//! Mask-then-compare contract (emRec.cpp:785-792): `value &= (1<<IdentifierCount)-1`,
//! then `if (Value!=value) { Value=value; Changed(); }` — undefined bits are stripped
//! before the no-change-skip check.

use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRec::emRec;
use crate::emRecNode::emRecNode;
use crate::emRecReader::{emRecReader, ElementType, RecIoError};
use crate::emRecWriter::emRecWriter;
use crate::emSignal::SignalId;

pub struct emFlagsRec {
    value: i32,
    default: i32,
    signal: SignalId,
    identifiers: Vec<String>,
    /// Reified aggregate-signal chain; see ADR 2026-04-21-phase-4b-listener-tree-adr.md.
    aggregate_signals: Vec<SignalId>,
    // TODO(phase-4b+): SetToDefault, IsSetToDefault, TryStartReading, serialization hooks per emRec.h.
}

impl emFlagsRec {
    // TODO(phase-4b): emFlagsRec(parent, varIdentifier, default, identifiers...) per emRec.h:660.
    /// C++ emFlagsRec::Init (emRec.cpp:897-917): max 32 identifiers; default value
    /// is masked at construction time.
    pub fn new<C: ConstructCtx>(ctx: &mut C, default: i32, identifiers: &[&str]) -> Self {
        assert!(
            !identifiers.is_empty(),
            "emFlagsRec: at least one identifier required (C++ ctor takes identifier0 + va_list)"
        );
        assert!(
            identifiers.len() <= 32,
            "emFlagsRec: Too many identifiers (max 32, got {})",
            identifiers.len()
        );
        let identifiers: Vec<String> = identifiers
            .iter()
            .map(|s| {
                check_identifier(s);
                (*s).to_string()
            })
            .collect();
        let mask = (1i32 << identifiers.len()) - 1;
        let masked_default = default & mask;
        Self {
            value: masked_default,
            default: masked_default,
            signal: ctx.create_signal(),
            identifiers,
            aggregate_signals: Vec::new(),
        }
    }

    pub fn GetIdentifierCount(&self) -> i32 {
        self.identifiers.len() as i32
    }

    /// C++ emFlagsRec::GetIdentifierOf (emRec.cpp:795-799): returns NULL when out of range.
    pub fn GetIdentifierOf(&self, bit: i32) -> Option<&str> {
        if bit < 0 || bit >= self.identifiers.len() as i32 {
            return None;
        }
        Some(self.identifiers[bit as usize].as_str())
    }

    /// C++ emFlagsRec::GetBitOf (emRec.cpp:802-810): backwards loop with `strcasecmp`.
    /// Returns `None` instead of -1 when not found.
    pub fn GetBitOf(&self, name: &str) -> Option<i32> {
        for bit in (0..self.identifiers.len()).rev() {
            if self.identifiers[bit].eq_ignore_ascii_case(name) {
                return Some(bit as i32);
            }
        }
        None
    }

    /// Port of C++ `emFlagsRec::TryStartWriting` (emRec.cpp:862-877).
    ///
    // DIVERGED: (language-forced) atomic fusion; see `emBoolRec::TryWrite` for rationale.
    // Format: `{` then set-bit identifiers separated by a single space, then
    // `}`. Empty (no bits set) yields `{}`.
    pub fn TryWrite(&self, writer: &mut dyn emRecWriter) -> Result<(), RecIoError> {
        writer.TryWriteDelimiter('{')?;
        let mut space_before_next = false;
        for bit in 0..self.identifiers.len() {
            if self.value & (1i32 << bit) != 0 {
                if space_before_next {
                    writer.TryWriteSpace()?;
                }
                writer.TryWriteIdentifier(&self.identifiers[bit])?;
                space_before_next = true;
            }
        }
        writer.TryWriteDelimiter('}')
    }

    /// Port of C++ `emFlagsRec::TryStartReading` (emRec.cpp:825-848).
    pub fn TryRead(
        &mut self,
        reader: &mut dyn emRecReader,
        ctx: &mut SchedCtx<'_>,
    ) -> Result<(), RecIoError> {
        let val = if reader.TryPeekNext()?.element_type() == ElementType::Int {
            let v = reader.TryReadInt()?;
            let mask = if self.identifiers.len() >= 32 {
                -1i32
            } else {
                (1i32 << self.identifiers.len()) - 1
            };
            if v & !mask != 0 {
                return Err(reader.ThrowElemError("Value out of range."));
            }
            v
        } else {
            reader.TryReadCertainDelimiter('{')?;
            let mut v = 0i32;
            while reader.TryPeekNext()?.element_type() == ElementType::Identifier {
                let idf = reader.TryReadIdentifier()?;
                match self.GetBitOf(&idf) {
                    Some(bit) => v |= 1i32 << bit,
                    None => return Err(reader.ThrowElemError("Unknown identifier.")),
                }
            }
            reader.TryReadCertainDelimiter('}')?;
            v
        };
        self.SetValue(val, ctx);
        Ok(())
    }
}

// TODO(phase-4b+): Centralize on `emRec::CheckIdentifier` once additional emRec types
// (`emEnumRec`, `emStructRec` field varIdentifiers, etc.) need the same predicate.
/// C++ emRec::CheckIdentifier (emRec.cpp:173-194): enforces `[A-Za-z_][A-Za-z0-9_]*`
/// and `emFatalError`s on violation. Empty string fails (first byte is 0, none of
/// letter/underscore). ASCII-only by design; works over bytes to mirror C++ char-by-char.
fn check_identifier(s: &str) {
    let bytes = s.as_bytes();
    let valid_first = !bytes.is_empty()
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_uppercase() || bytes[0] == b'_');
    let valid = valid_first
        && bytes[1..].iter().all(|&b| {
            b.is_ascii_lowercase() || b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'
        });
    if !valid {
        panic!("emRec: '{s}' is not a valid identifier.");
    }
}

impl emRecNode for emFlagsRec {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }

    fn register_aggregate(&mut self, sig: SignalId) {
        self.aggregate_signals.push(sig);
    }

    fn listened_signal(&self) -> SignalId {
        self.signal
    }

    fn TryRead(
        &mut self,
        reader: &mut dyn emRecReader,
        ctx: &mut SchedCtx<'_>,
    ) -> Result<(), RecIoError> {
        emFlagsRec::TryRead(self, reader, ctx)
    }

    fn TryWrite(&self, writer: &mut dyn emRecWriter) -> Result<(), RecIoError> {
        emFlagsRec::TryWrite(self, writer)
    }
}

impl emRec<i32> for emFlagsRec {
    fn GetValue(&self) -> &i32 {
        &self.value
    }

    /// C++ emFlagsRec::Set (emRec.cpp:785-792): mask undefined bits, then skip
    /// mutation+signal when the masked value matches current.
    fn SetValue(&mut self, value: i32, ctx: &mut SchedCtx<'_>) {
        let mask = (1i32 << self.identifiers.len()) - 1;
        let value = value & mask;
        if value != self.value {
            self.value = value;
            ctx.fire(self.signal);
            // DIVERGED: (language-forced) C++ emRec::Changed() (emRec.h:243 inline, delegates to emRec::ChildChanged at emRec.cpp:217) walks UpperNode
            // per-fire; Rust fires the reified aggregate chain. See ADR
            // 2026-04-21-phase-4b-listener-tree-adr.md.
            for sig in &self.aggregate_signals {
                ctx.fire(*sig);
            }
        }
    }

    fn GetDefaultValue(&self) -> &i32 {
        &self.default
    }

    fn GetValueSignal(&self) -> SignalId {
        self.signal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emClipboard::emClipboard;
    use crate::emEngineCtx::{DeferredAction, FrameworkDeferredAction, SchedCtx};
    use crate::emScheduler::EngineScheduler;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_sched_ctx<'a>(
        sched: &'a mut EngineScheduler,
        actions: &'a mut Vec<DeferredAction>,
        ctx_root: &'a Rc<crate::emContext::emContext>,
        cb: &'a RefCell<Option<Box<dyn emClipboard>>>,
        pa: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>,
    ) -> SchedCtx<'a> {
        SchedCtx {
            scheduler: sched,
            framework_actions: actions,
            root_context: ctx_root,
            framework_clipboard: cb,
            current_engine: None,
            pending_actions: pa,
        }
    }

    #[test]
    fn set_value_fires_signal() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emFlagsRec::new(&mut sc, 0, &["foo", "bar", "baz"]);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(0b101, &mut sc);

        assert!(sc.is_signaled(sig), "signal must fire when value changes");
        assert_eq!(*rec.GetValue(), 0b101);

        sc.remove_signal(sig);
    }

    #[test]
    fn aggregate_signal_fires_on_change() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emFlagsRec::new(&mut sc, 0, &["foo", "bar", "baz"]);
        let sig = rec.GetValueSignal();
        let agg = sc.create_signal();
        rec.register_aggregate(agg);

        rec.SetValue(0b101, &mut sc);

        assert!(sc.is_signaled(sig));
        assert!(sc.is_signaled(agg), "aggregate signal must fire");

        sc.remove_signal(sig);
        sc.remove_signal(agg);
    }

    #[test]
    fn aggregate_signal_does_not_fire_on_no_op() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emFlagsRec::new(&mut sc, 0b011, &["foo", "bar", "baz"]);
        let sig = rec.GetValueSignal();
        let agg = sc.create_signal();
        rec.register_aggregate(agg);

        rec.SetValue(0b011, &mut sc);

        assert!(!sc.is_signaled(sig));
        assert!(!sc.is_signaled(agg), "aggregate must NOT fire on no-op");
    }

    #[test]
    fn set_to_same_value_does_not_fire() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emFlagsRec::new(&mut sc, 0b011, &["foo", "bar", "baz"]);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(0b011, &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire when value is unchanged"
        );
        assert_eq!(*rec.GetValue(), 0b011);
    }

    #[test]
    fn undefined_bits_are_masked() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emFlagsRec::new(&mut sc, 0, &["a", "b", "c"]);
        let sig = rec.GetValueSignal();

        rec.SetValue(0xFFFF_FFFFu32 as i32, &mut sc);

        assert_eq!(*rec.GetValue(), 0b111, "only bits 0..2 should survive");
        assert!(sc.is_signaled(sig), "signal must fire (0 -> 7)");

        sc.remove_signal(sig);
    }

    #[test]
    fn mask_then_compare_no_spurious_fire() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emFlagsRec::new(&mut sc, 0, &["a", "b", "c"]);
        let sig = rec.GetValueSignal();

        // 8 (bit 3) masks to 0; current value is already 0 → no signal.
        rec.SetValue(8, &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire when masked value matches current"
        );
        assert_eq!(*rec.GetValue(), 0);
    }

    #[test]
    fn get_identifier_of_in_and_out_of_range() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let rec = emFlagsRec::new(&mut sc, 0, &["foo", "bar", "baz"]);

        assert_eq!(rec.GetIdentifierOf(0), Some("foo"));
        assert_eq!(rec.GetIdentifierOf(2), Some("baz"));
        assert_eq!(rec.GetIdentifierOf(-1), None);
        assert_eq!(rec.GetIdentifierOf(3), None);
        assert_eq!(rec.GetIdentifierOf(100), None);
    }

    #[test]
    fn get_bit_of_case_insensitive() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let rec = emFlagsRec::new(&mut sc, 0, &["foo", "bar", "baz"]);

        assert_eq!(rec.GetBitOf("FOO"), Some(0));
        assert_eq!(rec.GetBitOf("Bar"), Some(1));
        assert_eq!(rec.GetBitOf("baz"), Some(2));
    }

    #[test]
    fn get_bit_of_unknown_returns_none() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let rec = emFlagsRec::new(&mut sc, 0, &["foo", "bar", "baz"]);

        assert_eq!(rec.GetBitOf("quux"), None);
        assert_eq!(rec.GetBitOf(""), None);
    }

    struct SchedCtxParts {
        sched: EngineScheduler,
        actions: Vec<DeferredAction>,
        ctx_root: Rc<crate::emContext::emContext>,
        cb: RefCell<Option<Box<dyn emClipboard>>>,
        pa: Rc<RefCell<Vec<FrameworkDeferredAction>>>,
    }

    fn fresh_sched_ctx_parts() -> SchedCtxParts {
        SchedCtxParts {
            sched: EngineScheduler::new(),
            actions: Vec::new(),
            ctx_root: crate::emContext::emContext::NewRoot(),
            cb: RefCell::new(None),
            pa: Rc::new(RefCell::new(Vec::new())),
        }
    }

    #[test]
    #[should_panic(expected = "is not a valid identifier")]
    fn check_identifier_rejects_empty() {
        let SchedCtxParts {
            mut sched,
            mut actions,
            ctx_root,
            cb,
            pa,
        } = fresh_sched_ctx_parts();
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        let _ = emFlagsRec::new(&mut sc, 0, &[""]);
    }

    #[test]
    #[should_panic(expected = "is not a valid identifier")]
    fn check_identifier_rejects_leading_digit() {
        let SchedCtxParts {
            mut sched,
            mut actions,
            ctx_root,
            cb,
            pa,
        } = fresh_sched_ctx_parts();
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        let _ = emFlagsRec::new(&mut sc, 0, &["1foo"]);
    }

    #[test]
    #[should_panic(expected = "is not a valid identifier")]
    fn check_identifier_rejects_leading_dash() {
        let SchedCtxParts {
            mut sched,
            mut actions,
            ctx_root,
            cb,
            pa,
        } = fresh_sched_ctx_parts();
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        let _ = emFlagsRec::new(&mut sc, 0, &["-foo"]);
    }

    #[test]
    #[should_panic(expected = "is not a valid identifier")]
    fn check_identifier_rejects_internal_space() {
        let SchedCtxParts {
            mut sched,
            mut actions,
            ctx_root,
            cb,
            pa,
        } = fresh_sched_ctx_parts();
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        let _ = emFlagsRec::new(&mut sc, 0, &["foo bar"]);
    }

    #[test]
    #[should_panic(expected = "is not a valid identifier")]
    fn check_identifier_rejects_punctuation() {
        let SchedCtxParts {
            mut sched,
            mut actions,
            ctx_root,
            cb,
            pa,
        } = fresh_sched_ctx_parts();
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        let _ = emFlagsRec::new(&mut sc, 0, &["foo-bar"]);
    }

    #[test]
    #[should_panic(expected = "is not a valid identifier")]
    fn check_identifier_rejects_non_ascii() {
        let SchedCtxParts {
            mut sched,
            mut actions,
            ctx_root,
            cb,
            pa,
        } = fresh_sched_ctx_parts();
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        let _ = emFlagsRec::new(&mut sc, 0, &["foö"]);
    }

    #[test]
    fn check_identifier_accepts_grammar_valid_names() {
        let SchedCtxParts {
            mut sched,
            mut actions,
            ctx_root,
            cb,
            pa,
        } = fresh_sched_ctx_parts();
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        let rec = emFlagsRec::new(&mut sc, 0, &["_foo", "_", "foo_bar", "foo123", "A"]);
        assert_eq!(rec.GetIdentifierCount(), 5);
    }

    #[test]
    fn get_identifier_count_returns_count() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let rec = emFlagsRec::new(&mut sc, 0, &["a", "b", "c", "d", "e"]);
        assert_eq!(rec.GetIdentifierCount(), 5);
    }
}
