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

//! Actix-web API backend.
//!
//! [Actix-web](https://github.com/actix/actix-web) is an asynchronous backend
//! for HTTP API, based on the [Actix](https://github.com/actix/actix) framework.

pub use actix_cors::{Cors, CorsFactory};
pub use actix_web::{http::Method as HttpMethod, web::Payload, HttpRequest, HttpResponse};

use actix_web::{
    body::Body,
    error::ResponseError,
    http::header,
    web::{self, scope, Json, Query},
    FromRequest,
};
use futures::future::{Future, FutureExt, LocalBoxFuture};
use serde::{de::DeserializeOwned, Serialize};

use std::{fmt, sync::Arc};

use crate::{
    Actuality, AllowOrigin, ApiBackend, ApiScope, EndpointMutability, Error as ApiError,
    ExtendApiBackend, NamedWith,
};

/// Type alias for the inner `actix-web` HTTP requests handler.
pub type RawHandler = dyn Fn(HttpRequest, Payload) -> LocalBoxFuture<'static, Result<HttpResponse, actix_web::Error>>
    + 'static
    + Send
    + Sync;

/// Raw `actix-web` backend requests handler.
#[derive(Clone)]
pub struct RequestHandler {
    /// Endpoint name.
    pub name: String,
    /// Endpoint HTTP method.
    pub method: actix_web::http::Method,
    /// Inner handler.
    pub inner: Arc<RawHandler>,
}

impl fmt::Debug for RequestHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestHandler")
            .field("name", &self.name)
            .field("method", &self.method)
            .finish()
    }
}

/// API builder for the `actix-web` backend.
#[derive(Debug, Clone, Default)]
pub struct ApiBuilder {
    handlers: Vec<RequestHandler>,
}

impl ApiBuilder {
    /// Constructs a new backend builder instance.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ApiBackend for ApiBuilder {
    type Handler = RequestHandler;
    type Backend = actix_web::Scope;

    fn raw_handler(&mut self, handler: Self::Handler) -> &mut Self {
        self.handlers.push(handler);
        self
    }

    fn wire(&self, mut output: Self::Backend) -> Self::Backend {
        for handler in self.handlers.clone() {
            let inner = handler.inner;
            output = output.route(
                &handler.name,
                web::method(handler.method.clone())
                    .to(move |request, payload| inner(request, payload)),
            );
        }
        output
    }
}

impl ExtendApiBackend for actix_web::Scope {
    fn extend<'a, I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a ApiScope)>,
    {
        for item in items {
            self = self.service(item.1.actix_backend.wire(scope(&item.0)))
        }
        self
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let body = serde_json::to_value(&self.body).unwrap();
        let body = if body == serde_json::json!({}) {
            Body::Empty
        } else {
            serde_json::to_string(&self.body).unwrap().into()
        };

        let mut response = HttpResponse::build(self.http_code)
            .header(header::CONTENT_TYPE, "application/problem+json")
            .body(body);

        for (key, value) in self.headers.iter() {
            response.headers_mut().append(key.clone(), value.clone());
        }
        response
    }
}

/// Creates a `HttpResponse` object from the provided JSON value.
/// Depending on the `actuality` parameter value, the warning about endpoint
/// being deprecated can be added.
fn json_response<T: Serialize>(actuality: Actuality, json_value: T) -> HttpResponse {
    let mut response = HttpResponse::Ok();

    if let Actuality::Deprecated {
        ref discontinued_on,
        ref description,
    } = actuality
    {
        // There is a proposal for creating special deprecation header within HTTP,
        // but currently it's only a draft. So the conventional way to notify API user
        // about endpoint deprecation is setting the `Warning` header.
        let expiration_note = match discontinued_on {
            // Date is formatted according to HTTP-date format.
            Some(date) => format!(
                "The old API is maintained until {}.",
                date.format("%a, %d %b %Y %T GMT")
            ),
            None => "Currently there is no specific date for disabling this endpoint.".into(),
        };

        let mut warning_text = format!(
            "Deprecated API: This endpoint is deprecated, \
             see the service documentation to find an alternative. \
             {}",
            expiration_note
        );

        if let Some(description) = description {
            warning_text = format!("{} Additional information: {}.", warning_text, description);
        }

        let warning_string = create_warning_header(&warning_text);

        response.header(header::WARNING, warning_string);
    }

    response.json(json_value)
}

/// Formats warning string according to the following format:
/// "<warn-code> <warn-agent> \"<warn-text>\" [<warn-date>]"
/// <warn-code> in our case is 299, which means a miscellaneous persistent warning.
/// <warn-agent> is optional, so we set it to "-".
/// <warn-text> is a warning description, which is taken as an only argument.
/// <warn-date> is not required.
/// For details you can see RFC 7234, section 5.5: Warning.
fn create_warning_header(warning_text: &str) -> String {
    format!("299 - \"{}\"", warning_text)
}

impl From<EndpointMutability> for actix_web::http::Method {
    fn from(mutability: EndpointMutability) -> Self {
        match mutability {
            EndpointMutability::Immutable => actix_web::http::Method::GET,
            EndpointMutability::Mutable => actix_web::http::Method::POST,
        }
    }
}

/// Takes `HttpRequest` as a parameter and extracts query:
/// - If request is immutable, the query is parsed from query string,
/// - If request is mutable, the query is parsed from the request body as JSON.
async fn extract_query<Q>(
    request: HttpRequest,
    payload: Payload,
    mutability: EndpointMutability,
) -> Result<Q, actix_web::error::Error>
where
    Q: DeserializeOwned + 'static,
{
    match mutability {
        EndpointMutability::Immutable => Query::extract(&request)
            .await
            .map(Query::into_inner)
            .map_err(From::from),

        EndpointMutability::Mutable => Json::from_request(&request, &mut payload.into_inner())
            .await
            .map(Json::into_inner)
            .map_err(From::from),
    }
}

impl<Q, I, F, R> From<NamedWith<Q, I, R, F>> for RequestHandler
where
    F: Fn(Q) -> R + 'static + Clone + Send + Sync,
    Q: DeserializeOwned + 'static,
    I: Serialize + 'static,
    R: Future<Output = Result<I, crate::Error>>,
{
    fn from(f: NamedWith<Q, I, R, F>) -> Self {
        let handler = f.inner.handler;
        let actuality = f.inner.actuality;
        let mutability = f.mutability;
        let index = move |request: HttpRequest, payload: Payload| {
            let handler = handler.clone();
            let actuality = actuality.clone();

            async move {
                let query = extract_query(request, payload, mutability).await?;
                let response = handler(query).await.map_err(actix_web::Error::from)?;
                Ok(json_response(actuality, response))
            }
            .boxed_local()
        };

        Self {
            name: f.name,
            method: f.mutability.into(),
            inner: Arc::from(index) as Arc<RawHandler>,
        }
    }
}

impl From<&AllowOrigin> for CorsFactory {
    fn from(origin: &AllowOrigin) -> Self {
        match *origin {
            AllowOrigin::Any => Cors::new().finish(),
            AllowOrigin::Whitelist(ref hosts) => {
                let mut builder = Cors::new();
                for host in hosts {
                    builder = builder.allowed_origin(host);
                }
                builder.finish()
            }
        }
    }
}

impl From<AllowOrigin> for CorsFactory {
    fn from(origin: AllowOrigin) -> Self {
        Self::from(&origin)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn assert_responses_eq(left: HttpResponse, right: HttpResponse) {
        assert_eq!(left.status(), right.status());
        assert_eq!(
            left.headers().iter().collect::<Vec<_>>(),
            right.headers().iter().collect::<Vec<_>>()
        );
        assert_eq!(left.body().as_ref(), right.body().as_ref());
    }

    #[test]
    fn test_create_warning_header() {
        assert_eq!(
            &create_warning_header("Description"),
            "299 - \"Description\""
        );
    }

    #[test]
    fn json_responses() {
        use chrono::TimeZone;

        let actual_response = json_response(Actuality::Actual, 123);
        assert_responses_eq(actual_response, HttpResponse::Ok().json(123));

        let deprecated_response_no_deadline = json_response(
            Actuality::Deprecated {
                discontinued_on: None,
                description: None,
            },
            123,
        );
        let expected_warning_text = "Deprecated API: This endpoint is deprecated, \
             see the service documentation to find an alternative. \
             Currently there is no specific date for disabling this endpoint.";
        let expected_warning = create_warning_header(expected_warning_text);
        assert_responses_eq(
            deprecated_response_no_deadline,
            HttpResponse::Ok()
                .header(header::WARNING, expected_warning)
                .json(123),
        );

        let description = "Docs can be found on docs.rs".to_owned();
        let deprecated_response_with_description = json_response(
            Actuality::Deprecated {
                discontinued_on: None,
                description: Some(description),
            },
            123,
        );
        let expected_warning_text = "Deprecated API: This endpoint is deprecated, \
             see the service documentation to find an alternative. \
             Currently there is no specific date for disabling this endpoint. \
             Additional information: Docs can be found on docs.rs.";
        let expected_warning = create_warning_header(expected_warning_text);
        assert_responses_eq(
            deprecated_response_with_description,
            HttpResponse::Ok()
                .header(header::WARNING, expected_warning)
                .json(123),
        );

        let deadline = chrono::Utc.ymd(2020, 12, 31).and_hms(23, 59, 59);

        let deprecated_response_deadline = json_response(
            Actuality::Deprecated {
                discontinued_on: Some(deadline),
                description: None,
            },
            123,
        );
        let expected_warning_text = "Deprecated API: This endpoint is deprecated, \
             see the service documentation to find an alternative. \
             The old API is maintained until Thu, 31 Dec 2020 23:59:59 GMT.";
        let expected_warning = create_warning_header(expected_warning_text);
        assert_responses_eq(
            deprecated_response_deadline,
            HttpResponse::Ok()
                .header(header::WARNING, expected_warning)
                .json(123),
        );
    }

    #[test]
    fn api_error_to_http_response() {
        let response = ApiError::bad_request()
            .header(header::LOCATION, "location")
            .docs_uri("uri")
            .title("title")
            .detail("detail")
            .source("source")
            .error_code(42)
            .error_response();
        let body = crate::error::ErrorBody {
            docs_uri: "uri".into(),
            title: "title".into(),
            detail: "detail".into(),
            source: "source".into(),
            error_code: Some(42),
        };
        let expected = HttpResponse::build(crate::HttpStatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/problem+json")
            .header(header::LOCATION, "location")
            .body(serde_json::to_string(&body).unwrap());
        assert_responses_eq(response, expected);
    }

    #[test]
    fn api_error_to_http_response_without_body() {
        let response = ApiError::bad_request().error_response();
        let expected = HttpResponse::build(crate::HttpStatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/problem+json")
            .finish();
        assert_responses_eq(response, expected);
    }
}
