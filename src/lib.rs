//! An asynchronous imgur API wrapper using `hyper` 0.11 and `serde`.
#![feature(conservative_impl_trait)]
#![deny(missing_debug_implementations, missing_copy_implementations, trivial_casts,
       trivial_numeric_casts, unsafe_code, unused_import_braces, unused_qualifications)]
#![warn(missing_docs)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]
#![recursion_limit = "128"]

extern crate hyper;
extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate error_chain;
extern crate hyper_tls;
extern crate native_tls;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
// #[macro_use]
extern crate log;
extern crate env_logger;

mod errors {
    #![allow(unused_doc_comment)]
    use hyper;
    use serde_json;
    use native_tls;

    error_chain! {
        foreign_links {
            Tls(native_tls::Error);
            Hyper(hyper::Error);
            Serde(serde_json::Error);
            Imgur(super::ApiError);
        }
    }
}

use std::{error, fmt};

use hyper::{Client, Method, Request, Uri};
use hyper::client::HttpConnector;
use hyper::header::Authorization;
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Handle;
use futures::{future, Future, Stream};
use serde::de::DeserializeOwned;

pub use errors::{Error, Result};

const DEFAULT_THREADS: usize = 2;
const API: &str = "https://api.imgur.com/3";

type HttpsClient = Client<HttpsConnector<HttpConnector>>;

/// Main client type.
#[derive(Debug, Clone)]
pub struct ImgurClient {
    client: HttpsClient,
    client_id: String,
}

impl ImgurClient {
    /// Create a new `ImgurClient`.
    pub fn new(handle: &Handle, client_id: String) -> Result<ImgurClient> {
        let connector = HttpsConnector::new(DEFAULT_THREADS, handle)?;
        let client = Client::configure().connector(connector).build(handle);
        Ok(ImgurClient { client, client_id })
    }

    ///  Create a new `ImgurClient` with a supplied `hyper::Client`.
    pub fn with_client(client: HttpsClient, client_id: String) -> ImgurClient {
        ImgurClient { client, client_id }
    }

    fn get_with_header<T>(&self, url: Uri) -> impl Future<Item = T, Error = Error>
    where
        T: DeserializeOwned,
    {
        let mut request = Request::new(Method::Get, url);
        request
            .headers_mut()
            .set(Authorization(format!("Client-ID {}", self.client_id)));
        self.client.request(request).map_err(Error::from).and_then(
            |resp| {
                resp.body().map_err(Error::from).concat2().and_then(|body| {
                    future::result(serde_json::from_slice(&body).map_err(Error::from))
                })
            },
        )
    }

    /// Gets data for an image (`GET /image/<id>`)
    pub fn image(&self, id: &str) -> impl Future<Item = Response<Image>, Error = Error> {
        let url = format!("{}/image/{}", API, id).parse().unwrap();
        self.get_with_header(url)
    }

    /// Gets data for all the images in an album. (`GET /album/<album_id>/images`).
    pub fn album_images(
        &self,
        album_id: &str,
    ) -> impl Future<Item = Response<Vec<Image>>, Error = Error> {
        let url = format!("{}/album/{}/images", API, album_id)
            .parse()
            .unwrap();
        self.get_with_header(url)
    }
}

/// Wrapper type returned from all the web API methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response<T> {
    /// HTTP status of the response.
    pub status: usize,
    /// Whether the response succeeded.
    pub success: bool,
    /// Either data returned from the API or an error (see `ResponseData`)
    pub data: ResponseData<T>,
}

/// The `data` field of the JSON response can either be some data (e.g. data for an image)
/// or an error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseData<T> {
    /// Successful response.
    Success(T),
    /// Error response.
    Error(ApiError),
}

impl<T> ResponseData<T> {
    /// Converts `self` into a `Result`.
    pub fn into_result(self) -> Result<T> {
        match self {
            ResponseData::Success(v) => Ok(v),
            ResponseData::Error(e) => Err(e.into()),
        }
    }
}

/// Error data returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    /// Error description.
    pub error: String,
    /// Request URL.
    pub request: String,
    /// HTTP method used for the request.
    pub method: String,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Request {} {} failed: {}",
            self.method,
            self.request,
            self.error
        )
    }
}

impl error::Error for ApiError {
    fn description(&self) -> &str {
        &self.error
    }
}

/// Data returned for an image.
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub account_id: Option<String>,
    pub account_url: Option<String>,
    pub ad_type: usize,
    pub ad_url: String,
    pub animated: bool,
    pub bandwidth: usize,
    pub datetime: usize,
    pub description: Option<String>,
    pub favorite: bool,
    pub height: usize,
    pub id: String,
    pub in_gallery: bool,
    pub in_most_viral: bool,
    pub is_ad: bool,
    pub link: String,
    pub nsfw: Option<bool>,
    pub section: Option<String>,
    pub size: usize,
    pub tags: Vec<String>,
    pub title: Option<String>,
    pub views: usize,
    pub vote: Option<String>,
    pub width: usize,
}

#[cfg(test)]
mod tests {
    use std::env;

    use tokio_core::reactor::Core;

    use super::*;

    const CLIENT_ID: &str = include_str!("client_id.txt");

    #[test]
    fn get_image() {
        let mut core = Core::new().unwrap();
        let api = ImgurClient::new(&core.handle(), CLIENT_ID.into()).unwrap();
        let id = "PE2NI";
        let work = api.image(id);
        let resp = core.run(work).unwrap();
        assert_eq!(resp.data.into_result().unwrap().id, id);
    }

    #[test]
    fn get_album_images() {
        let mut core = Core::new().unwrap();
        let api = ImgurClient::new(&core.handle(), CLIENT_ID.into()).unwrap();
        let id = "cXz3n";
        let work = api.album_images(id);
        let resp = core.run(work).unwrap();
    }

    #[test]
    fn get_error() {
        env::set_var("RUST_LOG", "imgur_api=debug");
        ::env_logger::init();

        let mut core = Core::new().unwrap();
        let api = ImgurClient::new(&core.handle(), CLIENT_ID.into()).unwrap();
        let id = "cXz";
        let work = api.album_images(id);
        let resp = core.run(work).unwrap();
        assert!(resp.data.into_result().is_err());
    }
}
