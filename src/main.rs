use std::{env, io};

use serde_json::Value;

use crate::scraper::{CanvasScraper, TokenPair};

mod scraper;

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let canvas_token = env::var("CANVAS_TOKEN");

    let canvas_url = env::var("CANVAS_URL").unwrap_or(String::from("canvas.ust.hk"));

    let canvas = match canvas_token {
        Ok(token) => {
            CanvasScraper::new(&canvas_url, &token).unwrap()
        },
        Err(_) => {
            print!("Paste your qr code url: ");

            let mut url = String::new();
            let _ = io::stdin().read_line(&mut url);

            let (canvas, TokenPair { access_token , refresh_token }) = CanvasScraper::new_with_url(url).await.unwrap();

            println!("Access token: {}", access_token);
            println!("Refresh token: {}", refresh_token);
            canvas
        }
    };

    let user_data: Value = serde_json::from_str(&canvas.get_user_profile().await.unwrap()).unwrap();

    let user_id = &user_data["id"];

    println!("{}", user_data);
    println!("{}", canvas.get(vec!["api", "v1", "courses"]).await.unwrap().text().await.unwrap());

}
