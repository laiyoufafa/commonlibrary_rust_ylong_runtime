/*
 * Copyright (c) 2023 Huawei Device Co., Ltd.
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Defines errors that would be returned by the runtime during execution.

use std::fmt::{Debug, Display, Formatter};
use std::io;

/// Schedule errors during the execution.
pub struct ScheduleError {
    repr: Repr,
}

impl Debug for ScheduleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.repr, f)
    }
}

impl Display for ScheduleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.repr {
            Repr::Simple(kind) => write!(f, "{}", kind.as_str()),
            Repr::Custom(ref c) => write!(f, "{:?}: {}", c.kind, c.error),
        }
    }
}

enum Repr {
    Simple(ErrorKind),
    Custom(Box<Custom>),
}

impl Debug for Repr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            Repr::Simple(kind) => f.debug_tuple("Kind").field(&kind).finish(),
            Repr::Custom(ref c) => std::fmt::Debug::fmt(&c, f),
        }
    }
}

#[derive(Debug)]
struct Custom {
    kind: ErrorKind,
    error: Box<dyn std::error::Error + Send + Sync>,
}

/// All error types that could be returned during execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ErrorKind {
    /// Task already shut down error
    TaskShutdown,
    /// Blocking pool thread spawn error
    BlockSpawnErr,
    /// Creating netpollor thread error
    NetSpawnErr,
    /// Task already canceled error
    TaskCanceled,
    /// Task state invalid
    TaskStateInvalid,
    /// Panic during execution error
    Panic,
    /// Any other type errors
    Other,
}

impl ErrorKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match *self {
            ErrorKind::TaskShutdown => "task already get shutdown",
            ErrorKind::BlockSpawnErr => "blocking pool thread initialization failed",
            ErrorKind::NetSpawnErr => "net poller thread initialization failed",
            ErrorKind::TaskCanceled => "task already canceled",
            ErrorKind::TaskStateInvalid => "task state invalid",
            ErrorKind::Panic => "panic error",
            ErrorKind::Other => "other error",
        }
    }
}

impl From<ErrorKind> for ScheduleError {
    /// Turns ErrorKind into ScheduleError
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::error::{ErrorKind, ScheduleError};
    ///
    /// let task_shutdown = ErrorKind::TaskShutdown;
    /// let error = ScheduleError::from(task_shutdown);
    /// assert_eq!("task already get shutdown", format!("{}", error));
    /// ```
    fn from(kind: ErrorKind) -> Self {
        ScheduleError {
            repr: Repr::Simple(kind),
        }
    }
}

impl ScheduleError {
    /// User defined error with customized error msg.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::error::{ScheduleError, ErrorKind};
    ///
    /// // Able to create new error types directly from strings
    /// let custom_error = ScheduleError::new(ErrorKind::TaskShutdown, "task shutdown");
    ///
    /// // It is also possible to create new error types from other error types
    /// let custom_error2 = ScheduleError::new(ErrorKind::TaskShutdown, custom_error);
    /// ```
    pub fn new<E>(kind: ErrorKind, error: E) -> ScheduleError
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self::_new(kind, error.into())
    }

    fn _new(kind: ErrorKind, error: Box<dyn std::error::Error + Send + Sync>) -> ScheduleError {
        ScheduleError {
            repr: Repr::Custom(Box::new(Custom { kind, error })),
        }
    }

    /// Gets the interior error msg from user defined error.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::error::ScheduleError;
    /// use ylong_runtime::error::ErrorKind;
    ///
    /// fn print_error(err: ScheduleError) {
    ///     if let Some(inner_err) = err.into_inner() {
    ///         println!("Inner error: {}", inner_err);
    ///     } else {
    ///         println!("No inner error");
    ///     }
    /// }
    ///
    /// fn main() {
    ///     print_error(ScheduleError::new(ErrorKind::TaskShutdown, "oh no!"));
    /// }
    /// ```
    pub fn into_inner(self) -> Option<Box<dyn std::error::Error + Send + Sync>> {
        match self.repr {
            Repr::Simple(..) => None,
            Repr::Custom(c) => Some(c.error),
        }
    }

    /// Gets the ErrorKind of the error.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::error::ScheduleError;
    /// use ylong_runtime::error::ErrorKind;
    ///
    /// fn print_error(err: ScheduleError) {
    ///     println!("{:?}", err.kind());
    /// }
    ///
    /// fn main() {
    ///     print_error(ErrorKind::TaskShutdown.into());
    ///     print_error(ScheduleError::new(ErrorKind::TaskShutdown, "task shutdown"));
    /// }
    /// ```
    pub fn kind(&self) -> ErrorKind {
        match self.repr {
            Repr::Simple(kind) => kind,
            Repr::Custom(ref c) => c.kind,
        }
    }
}

impl From<ScheduleError> for io::Error {
    fn from(e: ScheduleError) -> Self {
        io::Error::new(io::ErrorKind::Other, e)
    }
}

impl std::error::Error for ScheduleError {}

#[cfg(all(test))]
mod test {
    use crate::error::{ErrorKind, ScheduleError};

    /*
     * @title  ScheduleError `Debug` UT test
     * @design The function has no input parameters, there is an exception branch, direct check function, return value
     * @precon None
     * @brief  Describe test case execution
     *         1、Creating simple errors
     *         2、Creating complex errors
     * @expect Verify that `Debug` prints for different error cases, and that they correspond to each other
     * @auto   true
     */
    #[test]
    fn ut_schedule_error_debug() {
        let simple_error: ScheduleError = ErrorKind::TaskShutdown.into();
        let custom_error = ScheduleError::new(ErrorKind::TaskShutdown, "task shutdown");

        assert_eq!(format!("{simple_error:?}"), "Kind(TaskShutdown)");
        assert_eq!(
            format!("{custom_error:?}"),
            "Custom { kind: TaskShutdown, error: \"task shutdown\" }"
        );
    }

    /*
     * @title  ScheduleError `Display` UT test
     * @design The function has no input parameters, there is an exception branch, direct check function, return value
     * @precon None
     * @brief  Describe test case execution
     *         1、Creating simple errors
     *         2、Creating complex errors
     * @expect Verify that `Display` prints for different error cases, and that they correspond to each other
     * @auto   true
     */
    #[test]
    fn ut_schedule_error_display() {
        let simple_error: ScheduleError = ErrorKind::TaskShutdown.into();
        let custom_error = ScheduleError::new(ErrorKind::TaskShutdown, "custom task shutdown");

        assert_eq!(format!("{simple_error}"), "task already get shutdown");
        assert_eq!(
            format!("{custom_error}"),
            "TaskShutdown: custom task shutdown"
        );
    }

    /*
     * @title  ScheduleError::new() UT test
     * @design The function has no invalid input, there is an exception branch, direct check function, return value
     * @precon None
     * @brief  Describe test case execution
     *         1、Creating simple errors
     *         2、Creating complex errors
     * @expect Verify the error structure created
     * @auto   true
     */
    #[test]
    fn ut_schedule_error_new() {
        let custom_error_one =
            ScheduleError::new(ErrorKind::Other, std::sync::mpsc::RecvTimeoutError::Timeout);
        assert_eq!(
            format!("{custom_error_one:?}"),
            "Custom { kind: Other, error: Timeout }"
        );
        assert_eq!(
            format!("{custom_error_one}"),
            "Other: timed out waiting on channel"
        );

        let custom_error_two = ScheduleError::new(ErrorKind::TaskShutdown, "task shutdown");
        assert_eq!(
            format!("{custom_error_two:?}"),
            "Custom { kind: TaskShutdown, error: \"task shutdown\" }"
        );
        assert_eq!(format!("{custom_error_two}"), "TaskShutdown: task shutdown");
    }

    /*
     * @title  ScheduleError::kind() UT test
     * @design The function has no input parameters, there is an exception branch, direct check function, return value
     * @precon None
     * @brief  Describe test case execution
     *         1、Creating simple errors
     *         2、Creating complex errors
     * @expect Verify the `ErrorKind` condition in the created error structure
     * @auto   true
     */
    #[test]
    fn ut_schedule_error_kind() {
        let simple_error: ScheduleError = ErrorKind::Other.into();
        let custom_error = ScheduleError::new(ErrorKind::TaskShutdown, "task shutdown");

        assert_eq!(format!("{:?}", simple_error.kind()), "Other");
        assert_eq!(format!("{:?}", custom_error.kind()), "TaskShutdown");
    }

    /*
     * @title  ScheduleError::into_inner() UT test
     * @design The function has no input parameters, there is an exception branch, direct check function, return value
     * @precon None
     * @brief  Describe test case execution
     *         1、Creating simple errors
     *         2、Creating complex errors
     * @expect Checking the results according to the different cases of creation errors
     * @auto   true
     */
    #[test]
    fn ut_schedule_error_into_inner() {
        let simple_error: ScheduleError = ErrorKind::Other.into();
        let custom_error = ScheduleError::new(ErrorKind::TaskShutdown, "task shutdown");

        assert!(simple_error.into_inner().is_none());
        assert_eq!(
            format!("{}", custom_error.into_inner().unwrap()),
            "task shutdown"
        );
    }
}
