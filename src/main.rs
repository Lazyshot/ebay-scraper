#[macro_use]
extern crate rocket;
#[macro_use]
extern crate log;
extern crate fern;

use std::collections::HashMap;

use rocket::{response::status::Custom, http::Status,futures::StreamExt, State};
use rocket::serde::{Deserialize, Serialize, json::Json};
use chromiumoxide::{browser::{Browser, BrowserConfig}, error::CdpError};
use scraper::{Html, Selector};


#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct Scrape<'r> {
    url: &'r str,
    until: Option<String>, // Scrape articles until this article url is seen (excluding this article)
    // download_images: Option<bool>, // Downloads images and returns them as base64 encoded instead of URLs
}

#[derive(Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct Article {
    url: String,
    title: String,
    location: String,
    price: String,
    description: String,
    details: HashMap<String, String>,
    images: Vec<String>,
}

#[derive(Debug, Clone)]
struct ArticleParseError {
    url: String,
}

impl std::fmt::Display for ArticleParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failure to parse item article: {}", self.url)
    }
}

impl Into<Custom<String>> for ArticleParseError {
    fn into(self) -> Custom<String> {
        Custom(Status::new(500), self.to_string())
    }
}

impl Article {
    fn parse_from_html(article_url: String, article_html: String) -> Result<Self, ArticleParseError> {
        let article_doc = Html::parse_document(article_html.as_str());

        let title_selector = Selector::parse("h1.boxedarticle--title").unwrap();
        let price_selector = Selector::parse("h2#viewad-price").unwrap();
        let location_selector = Selector::parse("span#viewad-locality").unwrap();
        let description_selector = Selector::parse("p#viewad-description-text").unwrap();
        let detail_selector = Selector::parse("li.addetailslist--detail").unwrap();
        let image_selector = Selector::parse("div.galleryimage-element img#viewad-image").unwrap();


        let title = article_doc.select(&title_selector).next().unwrap().last_child().unwrap().value().as_text().unwrap().trim().to_string();
        let price = article_doc.select(&price_selector).next().unwrap().first_child().unwrap().value().as_text().unwrap().trim().to_string();
        let location = article_doc.select(&location_selector).next().unwrap().first_child().unwrap().value().as_text().unwrap().trim().to_string();
        let description = article_doc.select(&description_selector).next().unwrap().text().collect::<String>().trim().to_string();

        let images = article_doc
            .select(&image_selector)
            .filter_map(|e| e.value().attr("src"))
            .map(|s| s.to_string())
            .collect();
        let details = article_doc
            .select(&detail_selector)
            .filter_map(|e| {
                Some((
                    e.text().nth(0)?.trim().to_string(), 
                    e.text().nth(1)?.trim().to_string()
                ))
            })
            .collect();
        

        Ok(Article{
            url: article_url,
            title,
            price,
            location,
            description,
            details,
            images,
        })
    }
}



#[post("/", data="<scrape>")]
async fn index(scrape: Json<Scrape<'_>>, browser: &State<Browser>) -> Result<Json<Vec<Article>>, Custom<String>> {
    let map_browser_err = |e: CdpError| Custom(Status::new(500), e.to_string());
    let page = browser.new_page(scrape.url).await.map_err(map_browser_err)?;
    let mut articles: Vec<Article> = Vec::new();

    page.wait_for_navigation().await.map_err(map_browser_err)?;

    'outer: loop {
        let html = page.content().await.map_err(map_browser_err)?;
        let article_urls: Vec<String>;
        let next_page: Option<String>;

        {
            let search_result_doc = Html::parse_document(html.as_str());
            let article_href_selector = Selector::parse("a.ellipsis").unwrap();
            let next_page_selector = Selector::parse("a.pagination-next").unwrap();

            article_urls = search_result_doc
                .select(&article_href_selector)
                .filter_map(|a| a.value().attr("href"))
                .map(|a| format!("https://www.ebay-kleinanzeigen.de{}",a))
                .collect();

            next_page = search_result_doc
                .select(&next_page_selector)
                .next()
                .and_then(|e| Some(e.value().attr("href")?.to_string()));

            info!("Found {} article urls", article_urls.len())
        }


        for article_url in article_urls {
            let article_url = &article_url;
            info!("fetching article: {}", article_url);

            if scrape.until.as_ref().unwrap_or(&"".to_string()).eq(article_url) {
                info!("found until url, stopping");
                break 'outer;
            }

            let page = page.goto(article_url).await.map_err(map_browser_err)?;

            info!("fetched article: {}", article_url);

            let article_html = page.content().await.map_err(map_browser_err)?;            

            articles.push(Article::parse_from_html(article_url.clone(), article_html).map_err(|e| Custom(Status::new(500), e.to_string()))?);
        }

        match next_page {
            None => break,
            Some(next_page_url) => { 
                info!("going to next page: {}", next_page_url);
                page.goto(format!("https://www.ebay-kleinanzeigen.de{}",next_page_url)).await.map_err(map_browser_err)?;
            }
        }
    }
    

    page.close().await.map_err(map_browser_err)?;

    Ok(Json(articles))
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger().unwrap();
    let ( browser, mut handler) =
        Browser::launch(BrowserConfig::builder().build()?).await.map_err(|e| { error!("error launching browser: {}", e); e })?;
    
    // spawn a new task that continuously polls the handler
    let handle = tokio::task::spawn(async move {
        loop {
            let _ = handler.next().await.unwrap();
        }
    });
    
    let _rocket = rocket::build()
        .mount("/", routes![index])
        .manage(browser)
        .launch()
        .await?;

    handle.abort();
    Ok(())
}

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}
