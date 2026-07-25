//! Test-only audit of bounded asset input opens.

use std::path::Path;

#[cfg(any(test, feature = "test-read-audit"))]
use std::cell::RefCell;

#[cfg(any(test, feature = "test-read-audit"))]
thread_local! {
    static TEST_INPUT_OPENS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-read-audit"))]
pub fn test_reset_input_opens() {
    TEST_INPUT_OPENS.with_borrow_mut(Vec::clear);
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-read-audit"))]
pub fn test_take_input_opens() -> Vec<String> {
    TEST_INPUT_OPENS.take()
}

#[cfg(any(test, feature = "test-read-audit"))]
pub(crate) fn record_test_input_open(path: &Path) {
    TEST_INPUT_OPENS.with_borrow_mut(|paths| {
        paths.push(path.to_string_lossy().into_owned());
    });
}

#[cfg(not(any(test, feature = "test-read-audit")))]
pub(crate) fn record_test_input_open(_path: &Path) {}
