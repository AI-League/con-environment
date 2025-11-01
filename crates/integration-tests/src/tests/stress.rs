use crate::client::TestClient;
use anyhow::Result;
pub async fn run_tests(client: &TestClient) -> Result<()> {
    test_multiple_concurrent_users(client).await?;
    Ok(())
}

async fn test_multiple_concurrent_users(client: &TestClient) -> Result<()> {
    let num_users = 5;
    
    // Step 1: Prepare all data upfront
    let mut requests = Vec::new();
    for i in 0..num_users {
        let user_id = format!("stress-user-{}", i);
        let token = client.generate_test_token(&user_id)?;  // Done before spawn
        let url = format!("{}/{}/test", client.hub_url(), client.hub_namespace());
        requests.push((url, token));
    }
    
    // Step 2: Create HTTP client once
    let http_client = reqwest::Client::new();
    
    // Step 3: Spawn tasks with owned data
    let mut handles = vec![];
    for (url, token) in requests {
        let http_client = http_client.clone();  // Cheap clone (Arc internally)
        
        let handle = tokio::spawn(async move {
            // All data is owned!
            http_client.get(&url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
        });
        
        handles.push(handle);
    }
    
    // Step 4: Wait for completion
    for handle in handles {
        handle.await??;
    }
    
    Ok(())
}