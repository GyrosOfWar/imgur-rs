#![feature(conservative_impl_trait)]

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
        }
    }
}

use hyper::{Client, Method, Request, Uri};
use hyper::client::HttpConnector;
use hyper::header::Authorization;
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Handle;
use futures::{future, Future, Stream};
use serde::de::DeserializeOwned;

use errors::{Error, Result};

const DEFAULT_THREADS: usize = 2;
const API: &str = "https://api.imgur.com/3";

pub type HttpsClient = Client<HttpsConnector<HttpConnector>>;

pub struct ImgurClient {
    client: HttpsClient,
    client_id: String,
}

impl ImgurClient {
    pub fn new(handle: &Handle, client_id: String) -> Result<ImgurClient> {
        let connector = HttpsConnector::new(DEFAULT_THREADS, handle)?;
        let client = Client::configure().connector(connector).build(handle);
        Ok(ImgurClient { client, client_id })
    }

    pub fn with_client(client: HttpsClient, client_id: String) -> ImgurClient {
        ImgurClient { client, client_id }
    }

    pub fn make_request<T>(&self, url: Uri) -> impl Future<Item = T, Error = Error>
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

    pub fn image(&self, id: &str) -> impl Future<Item = ImgurResponse<Image>, Error = Error> {
        let url = format!("{}/image/{}", API, id).parse().unwrap();
        self.make_request(url)
    }

    pub fn album_images(&self, album_id: &str) -> impl Future<Item = ImgurResponse<Vec<Image>>, Error = Error> {
        let url = format!("{}/album/{}/images", API, album_id).parse().unwrap();
        self.make_request(url)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImgurResponse<T> {
    pub status: usize,
    pub success: bool,
    pub data: T,
}

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
        assert_eq!(resp.data.id, id);
    }

    #[test]
    fn get_album_images() {
        let mut core = Core::new().unwrap();
        let api = ImgurClient::new(&core.handle(), CLIENT_ID.into()).unwrap();
        let id = "cXz3n";
        let work = api.album_images(id);
        let resp = core.run(work).unwrap();
    }
}
