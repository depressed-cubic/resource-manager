use anyhow::Error;
use anyhow::anyhow;
use futures::Stream;
use reqwest::{Client, IntoUrl, Response, Url, header::HeaderMap};
use serde::Deserialize;
use serde_json::Value;
// use tokio_stream::Stream;
// use tokio_stream::StreamExt;

const CANVAS_VERIFY_URL: &str = "https://canvas.instructure.com/api/v1/mobile_verify.json";
const USER_AGENT: &str = "candroid/6.19.0 (123456)";

pub struct CanvasScraper {
    domain: Url,
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

/// HELP ME
fn get_paginated_list(client: &Client, link: Url) -> impl Stream<Item = Value> {
    let init: (Option<Url>, Vec<Value>)= (Some(link), vec![]);
    futures::stream::unfold(init, |s| async {
        let (mut cur_link, mut cur_buf) = s;
        if cur_buf.is_empty() {
            if let Some(link) = &cur_link {

                let resp_content = client.get(link.to_string()).send().await.ok().inspect(|r| {
                    if let Some(Ok(str)) = r.headers().get("Link").map(|h| h.to_str()) {
                        cur_link = str.split_terminator(",").map(|link_rel_pair| { 
                            link_rel_pair.split_terminator("; ")
                        }).find_map(|mut pair| {
                            let link = pair.next();
                            let rel = pair.next();
                            match rel {
                                Some(r#"rel="next""#) => link.map(Url::parse).and_then(|r| r.ok()),
                                _ => None
                            }
                        }) 
                    } else {
                        cur_link = None
                    };
                }).map(|r| async { serde_json::from_str::<Value>(&r.text().await.ok().unwrap()) }).unwrap().await.map(|v| {
                    match v {
                        Value::Array(vec) => vec,
                        value => vec![value]
                    }
                }).unwrap_or_else(|_| vec![]);

                cur_buf.extend(resp_content);
                cur_buf.pop().map(|v| (v, (cur_link, cur_buf)))
            } else {
                None
            }
        } else {
            cur_buf.pop().map(|v| (v, (cur_link, cur_buf)))
        }
    })
}

impl CanvasScraper {

    pub fn new(domain: &str, access_token: &str) -> Result<Self, Error> {
        let domain_abs = if domain[..7] == *"https://" {
            domain
        } else { &format!("https://{}", domain)};
        Ok( CanvasScraper { domain: Url::parse(domain_abs)?, client: Client::builder().user_agent(USER_AGENT).default_headers({
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

    pub fn get_courses(&self) -> Result<impl Stream<Item = Value>, Error> {
        let mut url = self.domain.clone();
        url.path_segments_mut().map_err(|_| { anyhow!["url cannot be a base"] })?.extend(vec!["api", "v1", "courses"]);
        Ok(get_paginated_list(&self.client, url))
    }
}

