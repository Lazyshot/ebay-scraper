# Ebay (Kleinanzeigen) Scraper

This project is aimed at providing a single crawling endpoint, which will return an array of "Articles" given a search URL.

It will paginate search results and provide some information about each article found.

To continue to look like a human, it uses headless chrome.

This was written in Rust primarily to learn.

## Use Case

This is primarily in service of some automated workflow tooling (i.e. n8n, node-red, etc.) to perform
scheduled scraping and store results.

## API

Request
```
POST /

{
    "url": "<search url>",
    "until": "<article url to stop scanning (assumes search is newest first)>"
}
```

Response:
```
[
    {
        "url": "<article url>",
        "title": "Title of the item",
        "location": "Text description of location",
        "price": "Price VB",
        "description": "",
        "details": { // mapping of details or standard attributes
            "Detail1": "Ja",
            "Detail2": "Groß",
        },
        "images": [
            "https://ebay-kleinanzeigen.de/...JPG"
        ]
    }
]
```

