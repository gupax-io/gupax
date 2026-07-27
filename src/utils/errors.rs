//---------------------------------------------------------------------------------------------------- [ErrorState] struct
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ErrorButtons {
    YesQuit,
    UseDetectedLocalNode((u16, u16)),
    UseNonSyncedNode,
    StayQuit,
    TrayOnClose,
    ResetState,
    ResetNode,
    Okay,
    Quit,
    WindowsAdmin,
    Debug,
    WarnUpdate(WarnUpdateData),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarnUpdateData {
    pub yes_button: String,
    pub no_button: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorFerris {
    Happy,
    Cute,
    Oops,
    Error,
    Panic,
    #[cfg(target_os = "windows")]
    Admin,
}

pub struct ErrorState {
    pub error: bool,           // Is there an error?
    pub msg: String,           // What message to display?
    pub ferris: ErrorFerris,   // Which ferris to display?
    pub buttons: ErrorButtons, // Which buttons to display?
    pub quit_twice: bool, // This indicates the user tried to quit on the [ask_before_quit] screen
}

impl Default for ErrorState {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorState {
    pub fn new() -> Self {
        Self {
            error: false,
            msg: "Unknown Error".to_string(),
            ferris: ErrorFerris::Oops,
            buttons: ErrorButtons::Okay,
            quit_twice: false,
        }
    }

    // Convenience function to enable the [App] error state
    pub fn set(&mut self, msg: impl Into<String>, ferris: ErrorFerris, buttons: ErrorButtons) {
        if self.error {
            // If a panic error is already set and there isn't an [Okay] confirm or another [Panic], return
            if self.ferris == ErrorFerris::Panic
                && (buttons != ErrorButtons::Okay || ferris != ErrorFerris::Panic)
            {
                return;
            }
        }
        *self = Self {
            error: true,
            msg: msg.into(),
            ferris,
            buttons,
            quit_twice: false,
        };
    }

    // Just sets the current state to new, resetting it.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
