use std::pin::Pin;

use anyhow::Error;
use anyhow::anyhow;
use futures::Stream;
use futures::StreamExt;
use futures::stream;
use reqwest::{Client, IntoUrl, Response, Url, header::HeaderMap};
use serde::Deserialize;
use serde_json::Value;
// use tokio_stream::Stream;
// use tokio_stream::StreamExt;

const CANVAS_VERIFY_URL: &str = "https://canvas.instructure.com/api/v1/mobile_verify.json";
const USER_AGENT: &str = "candroid/6.19.0 (123456)";

pub struct CanvasScraper {
    domain: Url,
    access_token: String,
    client: Client
}

#[derive(Deserialize)]
struct MobileVerifyResult {
    client_id: String,
    client_secret: String,
    base_url: String
}

#[derive(Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String
}

// #[async_trait::async_trait]
// pub trait AsyncIter<'a, T> {
//     async fn next(&'a mut self) -> Option<T>; 
// }
//
// pub struct AsyncIterMap<'a, A, B, F> 
//     where F: FnOnce(A) -> B {
//     async_iter: Box<dyn AsyncIter<'a, A>>,
//     func: &'a F
// }
//
// #[async_trait]
// impl<'a, A, B, F: Fn(A) -> B> AsyncIter<'a, B> for AsyncIterMap<'a, A, B, F> {
//     async fn next(&'a mut self) -> Option<B> {
//         self.async_iter.next().await.map(self.func)
//     }
//
// }
//

pub struct PaginatedList<'a> {
    client: &'a Client,
    current_link: Option<Url>,
    cur_buf: Vec<Value>,
    cur_future: Option<Pin<Box<dyn Future<Output = Option<Value>> + Send>>>
}

// impl Stream for PaginatedList<'_> {
//     type Item = Value;
//
//     fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
//         // self.cur_future.map(|mut f| {
//         //     Future::poll(f.as_mut(), cx)
//         // }).unwrap_or_else(|| {
//         //         let kek = Box::pin(self.next());
//         //         self.get_mut().cur_future = Some(kek);
//         //         pin!(kek).poll(cx)
//         //     })
//         // match &mut self.cur_future {
//         //     Some(f) => Future::poll(f.as_mut(), cx),
//         //     None => {
//         //         let mut kek = Box::pin(self._next());
//         //         let result = pin![kek.as_mut()].poll(cx);
//         //         self.as_mut().cur_future = Some(kek);
//         //         result
//         //     }
//         // }
//         //
//         //
//     }
//
// }
//



/// HELP ME
pub fn get_paginated_list(client: &mut Client, link: Url) -> impl Stream<Item = Value> {
    // futures::stream::iter(0..).scan((Some(link), vec![]), |s, _| async {
    //     let (cur_link, cur_buf) = s;
    //     if cur_buf.is_empty() {
    //         if let Some(link) = cur_link {
    //             cur_buf = &mut client.get(link.to_string()).send().await.ok().inspect(|r| {
    //                 if let Some(Ok(str)) = r.headers().get("Link").map(|h| h.to_str()) {
    //                     cur_link = &mut str.split_terminator(",").map(|link_rel_pair| { 
    //                         link_rel_pair.split_terminator("; ")
    //                     }).find_map(|mut pair| {
    //                         let link = pair.next();
    //                         let rel = pair.next();
    //                         match rel {
    //                             Some(r#"rel="next""#) => link.map(Url::parse).and_then(|r| r.ok()),
    //                             _ => None
    //                         }
    //                     }) 
    //                 };
    //             }).map(|_| vec![]).unwrap_or(vec![]);
    //             todo!()
    //         } else {
    //             None
    //         }
    //     } else {
    //         None
    //     }
    // })
    stream::iter(0..).map(|_| Value::Null)
}

impl PaginatedList<'_> {
    // pub fn stream(&mut self) -> impl Stream<Item = Value> {
    //     futures::stream::iter(0..).scan(|_| {
    //
    //     })
    // }

    pub async fn _next(&mut self) -> Option<Value> {
        if self.cur_buf.is_empty() {
            if let Some(resp) = self.next_resp().await {
                resp.text().await.ok().map(|str| { serde_json::from_str::<Value>(&str) }).and_then(|result| result.ok()).and_then(|array| {
                    match array {
                        Value::Array(vec) => {
                            self.cur_buf = vec;
                            self.cur_buf.reverse();
                            self.cur_buf.pop()
                        },
                        a => {
                            Some(a)
                        }
                    }
                })
            } else {
                None 
            }
        } else { 
            self.cur_buf.pop()
        }
    }

    // I tried to made this as a async iterator but idk how to, this is my compromise
    async fn next_resp(&mut self) -> Option<Response> {
        if let Some(link) = &self.current_link {
            self.client.get(link.to_string()).send().await.ok().inspect(|r| {
                if let Some(Ok(str)) = r.headers().get("Link").map(|h| h.to_str()) {
                    self.current_link = str.split_terminator(",").map(|link_rel_pair| { 
                        link_rel_pair.split_terminator("; ")
                    }).find_map(|mut pair| {
                        let link = pair.next();
                        let rel = pair.next();
                        match rel {
                            Some(r#"rel="next""#) => link.map(Url::parse).and_then(|r| r.ok()),
                            _ => None
                        }
                    }) 
                };
            })
        } else {
            None
        }
    }

    fn new<'a>(client: &'a Client, link: Url) -> PaginatedList<'a> {
        PaginatedList {
            client,
            current_link: Some(link),
            cur_buf: vec![],
            cur_future: None
        }
    }
}

impl CanvasScraper {

    pub fn new(domain: &str, access_token: &str) -> Result<Self, Error> {
        let domain_abs = if domain[..7] == *"https://" {
            domain
        } else { &format!("https://{}", domain)};
        Ok( CanvasScraper { domain: Url::parse(domain_abs)?, access_token: access_token.to_string() , client: Client::builder().user_agent(USER_AGENT).default_headers({
            let mut headers = HeaderMap::new();
            headers.insert("Authorization", format!("Bearer {}", access_token).parse()?);
            headers
        }).build()? })
    }

    ///
    /// Create a new scraper by authorizing with the given qr_url
    ///
    pub async fn new_with_url<U: IntoUrl>(qr_url: U) -> Result<(Self, TokenPair), Error> {
        let client = Client::builder().user_agent(USER_AGENT).build()?;

        let kek = qr_url.into_url()?;

        let domain = kek.query_pairs().find_map(|pair| {
            let (q, v) = pair;
            if q == "domain" { Some(v) } else { None }
        }).ok_or( anyhow!["Invalid domain in qr url"] )?;
        let code = kek.query_pairs().find_map(|pair| {
            let (q, v) = pair;
            if q == "code_android" { Some(v) } else { None }
        }).ok_or( anyhow!["Invalid code in qr url"] )?;

        // Verify mobile
        let mut mobile_verify_url = Url::parse(CANVAS_VERIFY_URL)?;
        mobile_verify_url.query_pairs_mut()
            .append_pair("domain", &domain)
            .append_pair("user-agent", USER_AGENT);
        let verify_resp = client.get(mobile_verify_url).header("User-Agent", USER_AGENT).send().await?;
        let verify_result = verify_resp.json::<MobileVerifyResult>().await?;

        // Get token from qr code
        let mut oauth_url = Url::parse(&verify_result.base_url)?;

        oauth_url.path_segments_mut().map_err( |_| { anyhow!["Invalid base url received from mobile verify"] } )?
            .extend(["login", "oauth2", "token"]);

        let payload = format!(r#"{{
            "client_id": {client_id},
            "client_secret": "{client_secret}",
            "code": "{code}",
            "grant_type": "authorization_code",
            "redirect_uri": "urn:ietf:wg:oauth:2.0:oob"
        }}"#, client_id=verify_result.client_id, client_secret=verify_result.client_secret, code=code);

        let token_resp = client.post(oauth_url).body(payload).header("Content-Type", "application/json").send().await?;
        let token_text = token_resp.text().await?;
        let token: TokenPair = serde_json::from_str(&token_text)?;

        println!("{}", domain);
        Ok((Self::new(&domain, &token.access_token)?, token))
    }

    pub async fn get(&self, end_point: Vec<&str>) -> Result<Response, Error> {
        let mut domain = self.domain.clone();
        domain.path_segments_mut().map_err(|_| { anyhow!["Domain stored in client invalid"] })?
            .extend(end_point);
        Ok(self.client.get(domain).send().await?)
    }

    pub async fn post(&self, end_point: Vec<&str>) -> Result<Response, Error> {
        let mut domain = self.domain.clone();
        domain.path_segments_mut().map_err(|_| { anyhow!["Domain stored in client invalid"] })?
            .extend(end_point);
        Ok(self.client.post(domain).send().await?)

    }

    // pub async fn getPaginated
    //
    pub async fn get_user_profile(&self) -> Result<String, Error>{
        let res = self.get(vec!["api", "v1", "users", "self"]).await?;
        Ok(res.text().await?)
    }

    pub async fn get_courses<'a>(&'a self) -> Result<PaginatedList<'a>, Error> {
        let mut url = self.domain.clone();
        url.path_segments_mut().map_err(|_| { anyhow!["url cannot be a base"] })?.extend(vec!["api", "v1", "courses"]);
        Ok(PaginatedList::new(&self.client, url))
    }
}

