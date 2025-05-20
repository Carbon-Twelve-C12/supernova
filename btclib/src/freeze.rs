// This file is used to mark and disable problematic code sections during development
// When FREEZE_MODE is true, certain problematic components will be skipped from compilation

/// When true, problematic code sections marked with the freeze macro will be disabled
pub const FREEZE_MODE: bool = true;

/// Macro to conditionally compile code based on freeze status
#[macro_export]
macro_rules! freeze_skip {
    ($($tokens:tt)*) => {
        #[cfg(not(feature = "freeze"))]
        {
            $($tokens)*
        }
    };
}

/// Macro to generate empty implementations when in freeze mode
#[macro_export]
macro_rules! freeze_stub {
    ($type:ident, $func:ident, $ret:ty) => {
        #[cfg(feature = "freeze")]
        impl $type {
            pub fn $func(&self) -> $ret {
                Default::default()
            }
        }
    };
} 