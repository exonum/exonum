extern crate iron;
extern crate hyper;
extern crate unicase;

use self::iron::{ AfterMiddleware, headers};
use self::iron::method::Method;
use self::iron::method::Method::*;
use self::iron::prelude::*;
use self::iron::status::Status;
use self::unicase::UniCase;
use self::hyper::header::{Headers, AccessControlAllowOrigin};

pub type CORSEndpoint = (Vec<Method>, String);

pub struct CORS {
    pub origin: String,
    pub allowed_endpoints: Vec<CORSEndpoint>
}

impl CORS {
    #[allow(dead_code)]
    pub fn new(origin: String, endpoints: Vec<CORSEndpoint>) -> Self {
        CORS {
            origin: origin,
            allowed_endpoints: endpoints,
        }
    }

    pub fn is_allowed(&self, req: &mut Request) -> bool {
        let mut is_cors_endpoint = false;
        for endpoint in self.allowed_endpoints.clone() {
            let (methods, path) = endpoint;

            if !methods.contains(&req.method) &&
               req.method != Method::Options {
                continue;
            }

            let path: Vec<&str> = if path.starts_with('/') {
                path[1..].split('/').collect()
            } else {
                path[0..].split('/').collect()
            };

            if path.len() != req.url.path().len() {
                continue;
            }

            for (i, req_path) in req.url.path().iter().enumerate() {
                is_cors_endpoint = false;
                if *req_path != path[i] && !path[i].starts_with(':') {
                    break;
                }
                is_cors_endpoint = true;
            }
            if is_cors_endpoint {
                break;
            }
        }
        is_cors_endpoint
    }

    pub fn add_headers(&self, res: &mut Response, req: &Request) {
        res.headers.set(AccessControlAllowOrigin::Value(self.origin.clone()));

        res.headers.set(headers::AccessControlAllowHeaders(
            vec![
                UniCase(String::from("accept")),
                UniCase(String::from("authorization")),
                UniCase(String::from("content-type"))
            ]
        ));
        res.headers.set(headers::AccessControlAllowMethods(
            vec![Get, Post, Put, Delete]
        ));
        res.headers.set(headers::AccessControlAllowCredentials);
    }
}

impl AfterMiddleware for CORS {
    fn after(&self, req: &mut Request, mut res: Response) -> IronResult<Response> {

        if req.method == Method::Options {
            res = Response::with(Status::Ok);
        }

        //if self.is_allowed(req) {
            self.add_headers(&mut res, &req);
        //}

        Ok(res)
    }

    fn catch(&self, req: &mut Request, mut err: IronError)
        -> IronResult<Response> {
        if self.is_allowed(req) {
            self.add_headers(&mut err.response, &req);
        }
        Err(err)
    }
}