// Newtype IDs - repr(transparent) for zero-cost abstraction
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StepIdx(pub u16);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotIdx(pub u16);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExprIdx(pub u16);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionIdx(pub u16);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessorIdx(pub u16);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstIdx(pub u16);

// Engine signals
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineSignal {
    Continue,
    Yield,       // hit async boundary
    Finished,
    Failed { error_code: u16 },
}

// Engine errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    InvalidProgramCounter { step: StepIdx },
    SlotNotInitialized { slot: SlotIdx },
    SlotOutOfBounds { slot: SlotIdx, max: u16 },
    ExpressionTooComplex { instruction_count: u16 },
    DivisionByZero,
    TypeMismatch { expected: &'static str, actual: &'static str },
    ActionFailed { action: ActionIdx, code: u16 },
    InvalidJump { target: StepIdx },
}