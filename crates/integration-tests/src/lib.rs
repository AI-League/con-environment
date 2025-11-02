#[cfg(test)]
mod tests {
    use anyhow::Result;
    use fantoccini::{ClientBuilder, Locator};
    use regex::Regex;

    // Use the hostname from your .envrc file
    const HUB_URL: &str = "http://bigbertha.tail661e08.ts.net:8080";
    const GECKODRIVER_URL: &str = "http://localhost:4444";

    #[tokio::test]
    async fn test_full_user_login_workflow() -> Result<()> {
        // Connect to geckodriver (must be running)
        let mut caps = serde_json::json!({
            "browserName": "firefox",
            "moz:firefoxOptions": {
                "args": ["-headless"]
            }
        });
        let mut client = ClientBuilder::rustls()
            .expect("Unable to make a fantoccini client.")
            .capabilities(caps)
            .connect(GECKODRIVER_URL)
            .await
            .expect("Failed to connect to geckodriver");

        println!("Navigating to login page: {}", HUB_URL);
        client.goto(HUB_URL).await?;

        // 1. Check for the login form
        let h1 = client.find(Locator::Css("h1")).await?;
        assert_eq!(h1.text().await?, "Workshop Hub Login");

        // 2. Fill in the username and submit
        let username = format!("test-user-{}", std::time::UNIX_EPOCH.elapsed()?.as_millis());
        client.find(Locator::Css("input[name='username']")).await?
            .send_keys(&username).await?;
        client.find(Locator::Css("button[type='submit']")).await?
            .click().await?;

        // 3. Wait for the redirect to the proxied service
        // The URL will be /<workshop-name>/<user-id-slug>/...
        // We wait for the user-id-slug to appear in the URL
        let user_id_slug = Regex::new(r"[^a-zA-Z0-9-]")?.replace_all(&username, "").to_lowercase();
        let expected_url_pattern = format!("/{}/", user_id_slug);
        
        println!("Waiting for redirect to URL containing: {}", expected_url_pattern);
        client.wait(Regex::new(&expected_url_pattern)?).await?;

        // 4. Check that the proxied content is visible
        // The default image is 'nginxdemos/hello'
        let body = client.find(Locator::Css("body")).await?.text().await?;
        assert!(
            body.contains("Server address:"),
            "Proxied content 'Server address:' not found"
        );

        println!("Successfully logged in as {} and accessed workshop", username);
        
        // 5. Clean up
        client.close().await?;
        Ok(())
    }
}