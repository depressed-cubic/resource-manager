use anyhow::Error;
use anyhow::anyhow;
use reqwest::{Client, IntoUrl, Response, Url, header::HeaderMap};
use serde::Deserialize;

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

    pub async fn get_user_profile(&self) -> Result<String, Error>{
        let res = self.get(vec!["api", "v1", "users", "self"]).await?;
        Ok(res.text().await?)
    }
}

