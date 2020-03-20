// Copyright 2020 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Implementation of matcher for `ExecutionError`.

use std::fmt;

use super::{CallSite, CallType, ErrorKind, ExecutionError, ExecutionFail};

use crate::runtime::InstanceId;

/// Matcher for `ExecutionError`s that can have some fields unspecified. Can be compared to
/// an `ExceptionError`, e.g., in tests. The unspecified fields will match any value in the error.
///
/// # Examples
///
/// ```
/// use exonum::runtime::{ExecutionError, InstanceId, ErrorMatch};
/// use exonum_derive::ExecutionFail;
///
/// #[derive(Debug, ExecutionFail)]
/// pub enum Error {
///     /// Content hash already exists.
///     HashAlreadyExists = 0,
///     // other variants...
/// }
///
/// // Identifier of the service that will cause an error.
/// const SERVICE_ID: InstanceId = 100;
///
/// # fn not_run(error: ExecutionError) {
/// let err: &ExecutionError = // ...
/// #    &error;
/// let matcher = ErrorMatch::from_fail(&Error::HashAlreadyExists)
///     .for_service(SERVICE_ID);
/// assert_eq!(*err, matcher);
/// # }
/// ```
#[derive(Debug)]
pub struct ErrorMatch {
    kind: ErrorKind,
    description: StringMatch,
    runtime_id: Option<u32>,
    instance_id: Option<InstanceId>,
    call_type: Option<CallType>,
}

impl ErrorMatch {
    /// Creates a matcher from the provided error.
    ///
    /// The converted error has a kind and description set to the values returned
    /// by the corresponding methods of the [`ExecutionFail`] trait. The call site information
    /// (e.g., the instance ID) is not set.
    ///
    /// [`ExecutionFail`]: trait.ExecutionFail.html
    pub fn from_fail<F: ExecutionFail + ?Sized>(fail: &F) -> Self {
        Self::new(fail.kind(), fail.description())
    }

    /// Creates a matcher for `Unexpected` kind of errors.
    /// By default it will match any description.
    pub fn any_unexpected() -> Self {
        Self {
            kind: ErrorKind::Unexpected,
            description: StringMatch::Any,
            runtime_id: None,
            instance_id: None,
            call_type: None,
        }
    }

    pub(super) fn new(kind: ErrorKind, description: String) -> Self {
        Self {
            kind,
            description: StringMatch::Exact(description),
            runtime_id: None,
            instance_id: None,
            call_type: None,
        }
    }

    /// Accepts an error with any description.
    pub fn with_any_description(mut self) -> Self {
        self.description = StringMatch::Any;
        self
    }

    /// Accepts an error with any description containing the specified string.
    pub fn with_description_containing(mut self, pat: impl Into<String>) -> Self {
        self.description = StringMatch::Contains(pat.into());
        self
    }

    /// Accepts an error with any description matching the specified closure.
    pub fn with_description_matching<P>(mut self, pat: P) -> Self
    where
        P: Fn(&str) -> bool + 'static,
    {
        self.description = StringMatch::Generic(Box::new(pat));
        self
    }

    /// Accepts an error that has occurred in a runtime with the specified ID.
    pub fn in_runtime(mut self, runtime_id: u32) -> Self {
        self.runtime_id = Some(runtime_id);
        self
    }

    /// Accepts an error that has occurred in a service with the specified ID.
    pub fn for_service(mut self, instance_id: InstanceId) -> Self {
        self.instance_id = Some(instance_id);
        self
    }

    /// Accepts an error that has occurred in a call of the specified format.
    pub fn in_call(mut self, call_type: CallType) -> Self {
        self.call_type = Some(call_type);
        self
    }
}

impl PartialEq<ErrorMatch> for ExecutionError {
    fn eq(&self, error_match: &ErrorMatch) -> bool {
        let kind_matches = self.kind == error_match.kind;
        let runtime_matches = match (error_match.runtime_id, self.runtime_id) {
            (None, _) => true,
            (Some(match_id), Some(id)) => match_id == id,
            _ => false,
        };
        let instance_matches = match (error_match.instance_id, &self.call_site) {
            (None, _) => true,
            (Some(match_id), Some(CallSite { instance_id, .. })) => match_id == *instance_id,
            _ => false,
        };
        let call_type_matches = match (&error_match.call_type, &self.call_site) {
            (None, _) => true,
            (Some(match_type), Some(CallSite { call_type, .. })) => match_type == call_type,
            _ => false,
        };
        kind_matches
            && runtime_matches
            && instance_matches
            && call_type_matches
            && error_match.description.matches(&self.description)
    }
}

impl PartialEq<ExecutionError> for ErrorMatch {
    fn eq(&self, other: &ExecutionError) -> bool {
        other.eq(self)
    }
}

enum StringMatch {
    Any,
    Exact(String),
    Contains(String),
    Generic(Box<dyn Fn(&str) -> bool>),
}

impl fmt::Debug for StringMatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => formatter.write_str("Any"),
            Self::Exact(s) => formatter.debug_tuple("Exact").field(s).finish(),
            Self::Contains(s) => formatter.debug_tuple("Contains").field(s).finish(),
            Self::Generic(_) => formatter.debug_tuple("Generic").field(&"_").finish(),
        }
    }
}

impl StringMatch {
    pub(super) fn matches(&self, s: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(ref_str) => ref_str == s,
            Self::Contains(needle) => s.contains(needle),
            Self::Generic(match_fn) => match_fn(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::cognitive_complexity)] // More test code is fine
    fn execution_error_matching() {
        let mut error = ExecutionError {
            kind: ErrorKind::Unexpected,
            description: "Panic!".to_string(),
            runtime_id: None,
            call_site: None,
        };
        let mut matcher = ErrorMatch {
            kind: ErrorKind::Unexpected,
            description: StringMatch::Any,
            runtime_id: None,
            instance_id: None,
            call_type: None,
        };
        assert_eq!(error, matcher);

        // Check various description types.
        matcher.description = StringMatch::Exact("Panic!".to_owned());
        assert_eq!(error, matcher);
        matcher.description = StringMatch::Exact("Panic".to_owned());
        assert_ne!(error, matcher);
        matcher.description = StringMatch::Contains("nic!".to_owned());
        assert_eq!(error, matcher);
        matcher.description = StringMatch::Contains("nic?".to_owned());
        assert_ne!(error, matcher);
        matcher.description = StringMatch::Generic(Box::new(|s| s.eq_ignore_ascii_case("panic!")));
        assert_eq!(error, matcher);

        // Check `runtime_id` matching.
        error.runtime_id = Some(1);
        assert_eq!(error, matcher);
        matcher.runtime_id = Some(0);
        assert_ne!(error, matcher);
        matcher.runtime_id = Some(1);
        assert_eq!(error, matcher);

        // Check `instance_id` matching.
        error.call_site = Some(CallSite::new(100, CallType::Constructor));
        assert_eq!(error, matcher);
        matcher.instance_id = Some(99);
        assert_ne!(error, matcher);
        matcher.instance_id = Some(100);
        assert_eq!(error, matcher);

        // Check `call_type` matching.
        matcher.call_type = Some(CallType::AfterTransactions);
        assert_ne!(error, matcher);
        matcher.call_type = Some(CallType::Constructor);
        assert_eq!(error, matcher);

        error.call_site = Some(CallSite::new(
            100,
            CallType::Method {
                interface: "exonum.Configure".to_owned(),
                id: 1,
            },
        ));
        matcher.call_type = None;
        assert_eq!(error, matcher);
        matcher.call_type = Some(CallType::Method {
            interface: "exonum.Configure".to_owned(),
            id: 0,
        });
        assert_ne!(error, matcher);
        matcher.call_type = Some(CallType::Method {
            interface: "exonum.v2.Configure".to_owned(),
            id: 1,
        });
        assert_ne!(error, matcher);
        matcher.call_type = Some(CallType::Method {
            interface: "exonum.Configure".to_owned(),
            id: 1,
        });
        assert_eq!(error, matcher);
    }
}
