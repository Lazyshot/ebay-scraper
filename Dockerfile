FROM rust:1.67 as builder
WORKDIR /usr/src/ebay-scraper
COPY . .
RUN cargo install --path .

FROM debian:bullseye-slim

RUN apt-get update \
    && apt-get install -y wget gnupg \
    && wget -q -O - https://dl-ssl.google.com/linux/linux_signing_key.pub | gpg --dearmor -o /usr/share/keyrings/googlechrome-linux-keyring.gpg \
    && sh -c 'echo "deb [arch=amd64 signed-by=/usr/share/keyrings/googlechrome-linux-keyring.gpg] http://dl.google.com/linux/chrome/deb/ stable main" >> /etc/apt/sources.list.d/google.list' \
    && apt-get update \
    && apt-get install -y google-chrome-stable fonts-ipafont-gothic fonts-wqy-zenhei fonts-thai-tlwg fonts-khmeros fonts-kacst fonts-freefont-ttf libxss1 \
      --no-install-recommends \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r scraper && useradd -rm -g scraper -G audio,video scraper

USER scraper
WORKDIR /home/scraper

COPY --from=builder /usr/local/cargo/bin/ebay-scraper /usr/local/bin/ebay-scraper

CMD ["ebay-scraper"]

