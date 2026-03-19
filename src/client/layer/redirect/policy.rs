//! Tools for customizing the behavior of a [`FollowRedirect`][super::FollowRedirect] middleware.

use std::{fmt, pin::Pin};

use http::{HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode, Uri};

use crate::{Proxy, error::BoxError};

/// Trait for the policy on handling redirection responses.
pub trait Policy<B, E> {
    /// Invoked when the service received a response with a redirection status code (`3xx`).
    ///
    /// This method returns an [`Action`] which indicates whether the service should follow
    /// the redirection.
    fn redirect(&mut self, attempt: Attempt<'_>) -> Result<Action, E>;

    /// Returns whether redirection is currently permitted by this policy.
    ///
    /// This method is called to determine whether the client should follow redirects at all.
    /// It allows policies to enable or disable redirection behavior based on the [`Request`].
    fn follow_redirects(&mut self, _request: &mut Request<B>) -> bool;

    /// Invoked right before the service makes a [`Request`].
    fn on_request(&mut self, _request: &mut Request<B>);

    /// Invoked right after the service received a [`Response`].
    fn on_response<Body>(&mut self, _response: &mut Response<Body>);

    /// Try to clone a request body before the service makes a redirected request.
    fn clone_body(&self, _body: &B) -> Option<B>;
}

/// A type that holds information on a redirection attempt.
pub struct Attempt<'a> {
    pub(crate) status: StatusCode,
    pub(crate) headers: &'a HeaderMap,
    pub(crate) location: &'a Uri,
    pub(crate) previous: &'a Uri,
}

/// Proxy override for a redirect request.
#[derive(Debug)]
pub enum ProxyOverride {
    /// Use this proxy for the redirect.
    Use(Box<Proxy>),
    /// Go direct (clear any proxy).
    Direct,
}

/// Header overrides for a redirect request.
///
/// Each entry is `(name, value)`:
/// - `Some(value)` — set/override the header
/// - `None` — remove the header
///
/// Uses a `Vec` rather than a `HashMap` because redirect policies typically
/// override only a handful of headers, making linear scan cheaper than hashing.
pub(crate) type HeadersOverride = Vec<(HeaderName, Option<HeaderValue>)>;

/// A value returned by [`Policy::redirect`] which indicates the action
/// [`FollowRedirect`][super::FollowRedirect] should take for a redirection response.
pub enum Action {
    /// Follow the redirection, optionally with extra options to apply to the redirect request.
    Follow {
        /// Header overrides for the redirect request.
        headers_override: Option<HeadersOverride>,
        /// Proxy override for the redirect request.
        proxy_override: Option<ProxyOverride>,
    },
    /// Do not follow the redirection, and return the redirection response as-is.
    Stop,
    /// Pending async decision. The async task will be awaited to determine the final action.
    Pending(Pin<Box<dyn Future<Output = Action> + Send>>),
    /// An error occurred while determining the redirection action.
    Error(BoxError),
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Follow {
                headers_override,
                proxy_override,
            } => f
                .debug_struct("Follow")
                .field("headers_override", headers_override)
                .field("proxy_override", proxy_override)
                .finish(),
            Action::Stop => f.debug_tuple("Stop").finish(),
            Action::Pending(_) => f.debug_tuple("Pending").finish(),
            Action::Error(_) => f.debug_tuple("Error").finish(),
        }
    }
}
