// Copyright 2018 The Exonum Team
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

//! Iron-specific API handling.

extern crate bodyparser;
extern crate params;
extern crate router;

pub use iron::Handler;

use failure::Fail;
use iron::prelude::*;
use iron::status;
use router::Router;
use serde_json;

use std::sync::Arc;

use blockchain::ApiContext;
use super::ext::{ApiError, BoxedEndpoint, EndpointHolder, ServiceApi};

/// Response returned by the Iron adapter in case an endpoint
/// raises an error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error description.
    pub description: String,
    /// Debug information associated with the error.
    pub debug: String,
}

impl ErrorResponse {
    fn from_error(e: &ApiError) -> Self {
        ErrorResponse {
            description: e.to_string(),
            debug: format!("{:?}", e),
        }
    }
}

impl From<ApiError> for IronError {
    fn from(e: ApiError) -> IronError {
        use self::ApiError::*;

        let code = match e {
            UnknownId(..) | NotFound => status::NotFound,
            BadRequest(..) => status::BadRequest,
            VerificationFail(..) |
            TransactionNotSent(..) |
            InternalError(..) => status::InternalServerError,
        };
        let body = serde_json::to_string_pretty(&ErrorResponse::from_error(&e)).unwrap();
        IronError::new(e.compat(), (code, body))
    }
}

fn ok_response(response: &serde_json::Value) -> IronResult<Response> {
    use iron::headers::ContentType;
    use iron::modifiers::Header;

    let resp = Response::with((
        status::Ok,
        serde_json::to_string_pretty(response).unwrap(),
        Header(ContentType::json()),
    ));
    Ok(resp)
}

/// Transport adapter for HTTP that uses Iron framework.
#[derive(Debug)]
pub struct IronAdapter {
    context: ApiContext,
}

impl IronAdapter {
    /// Creates a new adapter.
    pub fn new(context: ApiContext) -> Self {
        IronAdapter { context }
    }

    /// Creates a handler.
    pub fn create_handler(&self, api: ServiceApi) -> Box<Handler> {
        // Can an endpoint be used in `GET` HTTP requests?
        fn can_get(e: &BoxedEndpoint) -> bool {
            e.readonly()
        }

        // Can an endpoint be used in `POST` HTTP requests?
        fn can_post(_: &BoxedEndpoint) -> bool {
            true
        }

        fn endpoint_from_req<'a, T: 'a>(
            api: &'a T,
            req: &mut Request,
        ) -> IronResult<&'a BoxedEndpoint>
        where
            T: EndpointHolder,
        {
            let params = req.extensions.get::<Router>().unwrap();
            let id = params.find("id").ok_or_else(
                || ApiError::UnknownId("".to_string()),
            )?;
            api.endpoint(id).ok_or_else(|| {
                ApiError::UnknownId(id.to_string()).into()
            })
        }

        let mut router = Router::new();
        let api = Arc::new(api);

        let get_api = Arc::clone(&api);
        let context = self.context.clone();
        let get_handler = move |req: &mut Request| {
            let get_api = get_api.filter(can_get);
            let endpoint = endpoint_from_req(&get_api, req)?;

            let map = req.get_ref::<params::Params>().unwrap();
            let query = match map.find(&["q"]) {
                None => serde_json::Value::Null,
                Some(&params::Value::String(ref query)) => {
                    serde_json::from_str(query).map_err(|e| {
                        ApiError::BadRequest(e.into())
                    })?
                }
                _ => {
                    return Err(ApiError::BadRequest("Request data is malformed".into()))?;
                }
            };

            let response = endpoint.with_context(&context).handle(query)?;
            ok_response(&response)
        };

        let post_api = Arc::clone(&api);
        let context = self.context.clone();
        let post_handler = move |req: &mut Request| {
            let post_api = post_api.filter(can_post);
            let endpoint = endpoint_from_req(&post_api, req)?;

            let query = match req.get::<bodyparser::Json>() {
                Ok(Some(body)) => body,
                _ => {
                    return Err(ApiError::BadRequest("Request body is malformed".into()))?;
                }
            };

            let response = endpoint.with_context(&context).handle(query)?;
            ok_response(&response)
        };

        router.get(":id", get_handler, "get_handler");
        router.post(":id", post_handler, "post_handler");

        Box::new(router)
    }
}
